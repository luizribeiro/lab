//! Shutdown supervisor: forwards SIGTERM/SIGINT from the lockin
//! parent to the sandbox wrapper's process group, waits for the
//! child to reap within a grace period, and escalates to SIGKILL
//! if it doesn't. Without this, lockin exits on the first signal
//! and the wrapper/child tree gets reparented to PID 1 and keeps
//! running (issue #10).

use std::process::ExitStatus;
use std::time::Duration;

use tokio::signal::unix::{signal, Signal, SignalKind};

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
/// window between spawn and supervise can't kill lockin via the
/// default disposition and orphan the freshly-forked child.
pub struct Signals {
    sigterm: Signal,
    sigint: Signal,
}

impl Signals {
    pub fn install() -> std::io::Result<Self> {
        Ok(Self {
            sigterm: signal(SignalKind::terminate())?,
            sigint: signal(SignalKind::interrupt())?,
        })
    }
}

/// Waits for `child` to exit while forwarding SIGTERM/SIGINT to its
/// process group (whose leader is `pid`, set up by the caller via
/// `setpgid(0, 0)` in a `pre_exec` hook).
///
/// `pid` doubles as the process-group id. We deliberately don't take
/// the pgid separately: `setpgid(0, 0)` makes the two equal by
/// construction, and accepting them as one value removes a footgun
/// where a caller could pass the raw pid and forget it isn't actually
/// a pgid leader.
pub async fn run(
    child: lockin::SandboxedChild,
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
