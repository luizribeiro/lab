//! Supervised execution of a [`SandboxedCommand`].
//!
//! Sets the sandbox wrapper into its own process group, hands the
//! controlling terminal's foreground pgrp to it for the duration of
//! the run, forwards SIGTERM/SIGINT from the parent to the child's
//! pgroup, and escalates to SIGKILL after a grace period if the child
//! ignores the forwarded signal. Designed so callers (`lockin run`,
//! the upcoming `lockin infer`) get the same supervision treatment
//! without re-implementing the pgrp/tcsetpgrp/SIGTTOU dance at every
//! call site (issue #10 plus the v0.3.0 fg-pgrp work).
//!
//! Available with the `tokio` feature.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::process::ExitStatus;
use std::time::Duration;

use anyhow::Context;
use tokio::signal::unix::{signal, Signal, SignalKind};

use crate::{SandboxedChild, SandboxedCommand};

/// Default grace period before escalating from SIGTERM/SIGINT to
/// SIGKILL. Generous enough for Python services (vllm, etc.) to
/// flush their own shutdown sequence.
const DEFAULT_SHUTDOWN_GRACE: Duration = Duration::from_secs(30);

/// Env var for tests (and operators with unusual workloads) to
/// override the grace period. Parsed as milliseconds.
const SHUTDOWN_GRACE_ENV: &str = "LOCKIN_SHUTDOWN_GRACE_MS";

fn shutdown_grace() -> Duration {
    match std::env::var(SHUTDOWN_GRACE_ENV)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
    {
        Some(ms) => Duration::from_millis(ms),
        None => DEFAULT_SHUTDOWN_GRACE,
    }
}

/// Pre-acquired SIGTERM/SIGINT streams. Must be created **before**
/// the sandbox child is spawned so a signal arriving in the narrow
/// window between spawn and supervise can't kill the parent via the
/// default disposition and orphan the freshly-forked child.
struct Signals {
    sigterm: Signal,
    sigint: Signal,
}

impl Signals {
    fn install() -> std::io::Result<Self> {
        Ok(Self {
            sigterm: signal(SignalKind::terminate())?,
            sigint: signal(SignalKind::interrupt())?,
        })
    }
}

/// Spawn `cmd` and supervise it to completion, returning the child's
/// exit status. The caller owns any pre-spawn configuration (env,
/// args, fds); this helper layers the process-group / controlling-tty
/// / signal-forwarding behavior on top.
///
/// Steps:
/// 1. Install a `pre_exec` `setpgid(0, 0)` so the wrapper becomes the
///    leader of a fresh pgroup whose pgid equals its pid.
/// 2. Open `/dev/tty` (best-effort: non-interactive runs skip the
///    `tcsetpgrp` dance).
/// 3. Ignore SIGTTOU / SIGTTIN in the parent so the kernel doesn't
///    suspend us when we hand fg ownership back at the end.
/// 4. Install SIGTERM/SIGINT handlers via tokio **before** spawning,
///    closing the spawn→supervise window where a signal could reach
///    SIG_DFL and orphan the tree.
/// 5. Spawn, idempotently `setpgid` from the parent, `tcsetpgrp` to
///    the child, run the supervisor loop, and restore fg ownership.
/// 6. Restore the SIGTTOU/SIGTTIN dispositions.
pub fn supervise_command(
    mut cmd: SandboxedCommand,
    runtime: &tokio::runtime::Handle,
) -> anyhow::Result<ExitStatus> {
    place_child_in_own_pgroup(&mut cmd);

    // None for non-interactive runs (CI, cron, redirected stdin);
    // skips the whole pgrp dance.
    let tty_fd = open_controlling_tty();

    // Without SIG_IGN, the kernel suspends *us* on the restoring
    // tcsetpgrp below — by then we've handed fg ownership away.
    let saved_sigttou = ignore_signal(libc::SIGTTOU)?;
    let saved_sigttin = ignore_signal(libc::SIGTTIN)?;

    let status = runtime
        .block_on(async {
            let signals = Signals::install()?;
            let child = cmd.spawn()?;
            let pid = child.id() as i32;
            // pre_exec calls setpgid in the child, but the parent
            // races to call tcsetpgrp. setpgid is idempotent — call
            // it from the parent too so the pgrp definitely exists
            // by the time we tcsetpgrp. Errors are expected (EACCES
            // if the child already exec'd; ESRCH if it exited) and
            // safe to ignore.
            unsafe { libc::setpgid(pid, pid) };
            if let Some(fd) = tty_fd.as_ref() {
                if unsafe { libc::tcsetpgrp(fd.as_raw_fd(), pid) } == -1 {
                    eprintln!(
                        "lockin: tcsetpgrp(child) failed: {}",
                        std::io::Error::last_os_error()
                    );
                }
            }
            let result = supervisor_loop(child, pid, signals).await;
            if let Some(fd) = tty_fd.as_ref() {
                let our_pgrp = unsafe { libc::getpgrp() };
                unsafe { libc::tcsetpgrp(fd.as_raw_fd(), our_pgrp) };
            }
            result
        })
        .context("supervising sandbox child")?;

    restore_signal(libc::SIGTTOU, &saved_sigttou);
    restore_signal(libc::SIGTTIN, &saved_sigttin);

    Ok(status)
}

/// Waits for `child` to exit while forwarding SIGTERM/SIGINT to its
/// process group (whose leader is `pid`, set up via `setpgid(0, 0)`
/// in the `pre_exec` hook).
///
/// `pid` doubles as the process-group id. We deliberately don't take
/// the pgid separately: `setpgid(0, 0)` makes the two equal by
/// construction, and accepting them as one value removes a footgun
/// where a caller could pass the raw pid and forget it isn't actually
/// a pgid leader.
async fn supervisor_loop(
    child: SandboxedChild,
    pid: i32,
    mut signals: Signals,
) -> std::io::Result<ExitStatus> {
    let (mut std_child, _sandbox) = child.into_parts();

    // `std::process::Child::wait` is blocking and we need to race it
    // against signal arrivals on the runtime. Move it onto a
    // dedicated OS thread and deliver the result via oneshot.
    let (tx, mut wait_rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let _ = tx.send(std_child.wait());
    });

    let forwarded = tokio::select! {
        result = &mut wait_rx => return result.expect("wait thread must send exactly once"),
        _ = signals.sigterm.recv() => libc::SIGTERM,
        _ = signals.sigint.recv() => libc::SIGINT,
    };

    forward_to_pgroup(pid, forwarded);

    let grace = shutdown_grace();
    match tokio::time::timeout(grace, &mut wait_rx).await {
        Ok(result) => result.expect("wait thread must send exactly once"),
        Err(_) => {
            eprintln!(
                "lockin: child pgroup {pid} did not exit within {grace:?} of forwarded signal; sending SIGKILL"
            );
            forward_to_pgroup(pid, libc::SIGKILL);
            wait_rx.await.expect("wait thread must send exactly once")
        }
    }
}

fn forward_to_pgroup(pid: i32, sig: libc::c_int) {
    // SAFETY: killpg is async-signal-safe. ESRCH (group already
    // exited) is expected during shutdown races and is benign.
    unsafe {
        libc::killpg(pid, sig);
    }
}

/// Adds a `pre_exec` hook that puts the sandbox wrapper (syd /
/// sandbox-exec) into a fresh process group equal to its own PID.
/// That makes `killpg(pid, sig)` from the supervisor reach the
/// wrapper and every grand-child it spawns, so signal forwarding on
/// shutdown does not orphan the tree (issue #10).
fn place_child_in_own_pgroup(cmd: &mut SandboxedCommand) {
    // SAFETY: setpgid is async-signal-safe per POSIX. The hook does
    // no allocation and touches no shared state.
    unsafe {
        cmd.pre_exec(|| {
            if libc::setpgid(0, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

fn open_controlling_tty() -> Option<OwnedFd> {
    let fd = unsafe { libc::open(c"/dev/tty".as_ptr(), libc::O_RDWR | libc::O_NOCTTY) };
    if fd < 0 {
        None
    } else {
        Some(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

fn ignore_signal(sig: libc::c_int) -> std::io::Result<libc::sigaction> {
    let mut old: libc::sigaction = unsafe { std::mem::zeroed() };
    let mut new: libc::sigaction = unsafe { std::mem::zeroed() };
    new.sa_sigaction = libc::SIG_IGN;
    if unsafe { libc::sigaction(sig, &new, &mut old) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(old)
}

fn restore_signal(sig: libc::c_int, prev: &libc::sigaction) {
    if unsafe { libc::sigaction(sig, prev, std::ptr::null_mut()) } != 0 {
        eprintln!(
            "lockin: failed to restore signal {sig} disposition: {}",
            std::io::Error::last_os_error()
        );
    }
}
