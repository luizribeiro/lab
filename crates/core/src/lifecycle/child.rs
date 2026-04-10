//! Generic child-process primitives used by `super::netd` and
//! `super::vmm`. The only file in `lifecycle/` that knows nothing
//! about VMs: it speaks `Child`, `pid`, signals, and reaper threads.
//!
//! Intentionally small and concrete: two daemon binaries do not
//! justify a trait hierarchy. Don't reach for crossbeam, async, or
//! pidfd here unless a real requirement forces the change.
//!
//! The ownership story:
//!
//! - `spawn_sandboxed` forks a child, captures its pid, and immediately
//!   moves the `std::process::Child` into a single *reaper* thread that
//!   blocks in `Child::wait`. This thread is the one and only piece of
//!   code that ever calls `waitpid` for this child. It publishes the
//!   exit status on a `sync_channel(1)` and flips an `AtomicBool` so
//!   the rest of the program can observe that the child has been
//!   reaped.
//! - `ChildHandle` holds the pid, the atomic flag, the receiver, the
//!   reaper's `JoinHandle`, and (optionally) the `Sandbox` value whose
//!   private tmpdir must outlive the child.
//! - `Drop` sends `SIGTERM`/`SIGKILL` only while the atomic flag is
//!   `false`, so we never signal a PID that may have been reused by
//!   the kernel for an unrelated process. A narrow race between the
//!   "load flag" and "send signal" is possible but vanishingly
//!   unlikely in practice; see the comment on `Drop` for details.
//! - `wait_either` polls both handles' receivers with `try_recv` and a
//!   short sleep. No bridge threads, no signal handlers, no
//!   platform-specific syscalls.

use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use capsa_sandbox::{Sandbox, SandboxBuilder};

pub(super) const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);
pub(super) const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(25);

/// Resolve a daemon binary path. Checks, in order:
///   1. `env_override` (an environment variable name like `CAPSA_VMM_PATH`)
///   2. a sibling file next to the current executable
///   3. `PATH`
///
/// The resolved candidate must be a regular file with at least one
/// executable bit set. Directories and non-executable files are
/// rejected with a clear error.
pub(super) fn resolve_binary(env_override: &str, binary_name: &str) -> Result<PathBuf> {
    if let Some(raw) = std::env::var_os(env_override) {
        let candidate = PathBuf::from(raw);
        ensure_executable(&candidate).with_context(|| {
            format!(
                "{env_override} points to {} but it is not an executable file",
                candidate.display()
            )
        })?;
        return Ok(candidate);
    }

    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(dir) = current_exe.parent() {
            let candidate = dir.join(binary_name);
            if is_executable_file(&candidate) {
                return Ok(candidate);
            }
        }
    }

    if let Some(path_var) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path_var) {
            let candidate = dir.join(binary_name);
            if is_executable_file(&candidate) {
                return Ok(candidate);
            }
        }
    }

    bail!(
        "failed to locate executable `{binary_name}` \
         (set {env_override} or place it next to the current exe or on PATH)"
    )
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    meta.is_file() && (meta.permissions().mode() & 0o111) != 0
}

fn ensure_executable(path: &Path) -> Result<()> {
    let meta =
        std::fs::metadata(path).with_context(|| format!("cannot stat {}", path.display()))?;
    if !meta.is_file() {
        bail!("{} is not a regular file", path.display());
    }
    if (meta.permissions().mode() & 0o111) == 0 {
        bail!("{} is not executable (no x bit set)", path.display());
    }
    Ok(())
}

/// Which of the two children `wait_either` saw exit first.
pub(super) enum Exited {
    First(Result<ExitStatus>),
    Second(Result<ExitStatus>),
}

/// Handle to a spawned daemon child.
///
/// Created by [`spawn_sandboxed`]. The caller should either call
/// [`ChildHandle::shutdown`] (explicit teardown) or let the handle
/// drop (implicit teardown that logs any errors via `tracing::warn`).
pub(super) struct ChildHandle {
    name: &'static str,
    pid: i32,
    exited: Arc<AtomicBool>,
    status_rx: Receiver<Result<ExitStatus>>,
    reaper: Option<JoinHandle<()>>,
    // Keeps the sandbox's private tmpdir alive for the lifetime of the
    // child. Must drop AFTER the reaper joins, so field order matters
    // if we ever change this to use field-order drop semantics. Right
    // now `Drop::drop` explicitly joins the reaper first.
    _sandbox: Option<Sandbox>,
    shutdown_timeout: Duration,
    poll_interval: Duration,
}

impl ChildHandle {
    #[cfg(test)]
    pub(super) fn name(&self) -> &'static str {
        self.name
    }

    #[cfg(test)]
    pub(super) fn has_exited(&self) -> bool {
        self.exited.load(Ordering::Acquire)
    }

    /// Block until the child exits on its own and return its status.
    /// Does **not** send any signals. Use this when the caller expects
    /// the child to exit without intervention; for teardown use
    /// [`shutdown`](Self::shutdown) instead.
    pub(super) fn wait(mut self) -> Result<ExitStatus> {
        let result = self
            .status_rx
            .recv()
            .with_context(|| format!("{} reaper thread dropped its result channel", self.name))?;
        if let Some(reaper) = self.reaper.take() {
            if reaper.join().is_err() {
                tracing::warn!(daemon = self.name, "reaper thread panicked");
            }
        }
        result
    }

    /// Explicit teardown: request shutdown if the child is still
    /// running, wait for the reaper, and return its exit status.
    /// Production teardown happens via `Drop`, which uses the same
    /// `kill_with_timeout` helper.
    #[cfg(test)]
    pub(super) fn shutdown(mut self) -> Result<ExitStatus> {
        if !self.exited.load(Ordering::Acquire) {
            kill_with_timeout(
                self.pid,
                &self.exited,
                self.shutdown_timeout,
                self.poll_interval,
            )
            .with_context(|| format!("failed to shut down {} daemon", self.name))?;
        }

        let status_result = self
            .status_rx
            .recv()
            .with_context(|| format!("{} reaper thread dropped its result channel", self.name))?;

        if let Some(reaper) = self.reaper.take() {
            if reaper.join().is_err() {
                tracing::warn!(daemon = self.name, "reaper thread panicked");
            }
        }

        status_result
    }
}

impl Drop for ChildHandle {
    fn drop(&mut self) {
        // PID-reuse race: we only send signals while `exited == false`.
        // At that point the reaper is still blocked in `Child::wait`,
        // so the kernel has not yet reaped this pid and cannot have
        // reused it. Between the atomic load and the `kill` syscall
        // the reaper *could* return and the kernel *could* reuse the
        // pid, but the window is a few instructions wide and the
        // kernel does not aggressively recycle pids. Fully closing
        // this race would require pidfd_send_signal on Linux, which
        // we skip to keep the implementation portable.
        if !self.exited.load(Ordering::Acquire) {
            if let Err(err) = kill_with_timeout(
                self.pid,
                &self.exited,
                self.shutdown_timeout,
                self.poll_interval,
            ) {
                tracing::warn!(
                    daemon = self.name,
                    pid = self.pid,
                    error = %err,
                    "drop-time shutdown failed"
                );
            }
            // Drain the reaper's status message so it doesn't linger
            // in the channel buffer.
            let _ = self.status_rx.try_recv();
        }

        if let Some(reaper) = self.reaper.take() {
            if reaper.join().is_err() {
                tracing::warn!(daemon = self.name, "reaper thread panicked during drop");
            }
        }
    }
}

/// Spawns `binary` under a `capsa_sandbox` sandbox (or bypassed via
/// `CAPSA_DISABLE_SANDBOX`), starts a reaper thread, and returns a
/// [`ChildHandle`]. The passed `SandboxBuilder` should already carry
/// any inherited fds and policy the child needs.
pub(super) fn spawn_sandboxed(
    name: &'static str,
    binary: &Path,
    builder: SandboxBuilder,
    args: &[String],
    stdin_null: bool,
) -> Result<ChildHandle> {
    let (child, sandbox) = build_and_spawn(name, binary, builder, args, stdin_null)?;
    let pid = child.id() as i32;

    let exited = Arc::new(AtomicBool::new(false));
    let (status_tx, status_rx) = mpsc::sync_channel::<Result<ExitStatus>>(1);
    let reaper = spawn_reaper(name, child, exited.clone(), status_tx);

    tracing::info!(daemon = name, pid, "spawned");

    Ok(ChildHandle {
        name,
        pid,
        exited,
        status_rx,
        reaper: Some(reaper),
        _sandbox: sandbox,
        shutdown_timeout: DEFAULT_SHUTDOWN_TIMEOUT,
        poll_interval: DEFAULT_POLL_INTERVAL,
    })
}

fn build_and_spawn(
    name: &'static str,
    binary: &Path,
    builder: SandboxBuilder,
    args: &[String],
    stdin_null: bool,
) -> Result<(Child, Option<Sandbox>)> {
    if sandbox_bypassed_by_env() {
        tracing::warn!(
            daemon = name,
            program = %binary.display(),
            "sandbox disabled via CAPSA_DISABLE_SANDBOX; running without sandbox"
        );
        let mut command = Command::new(binary);
        command.args(args);
        if stdin_null {
            command.stdin(Stdio::null());
        }
        let (inherited_fds, close_non_inherited, rlimits) = builder.into_bypass_config();
        capsa_sandbox::configure_inherited_fds(&mut command, inherited_fds, close_non_inherited)?;
        capsa_sandbox::configure_rlimits(&mut command, rlimits)?;
        let child = command.spawn().with_context(|| {
            format!("failed to spawn {name} daemon binary {}", binary.display())
        })?;
        return Ok((child, None));
    }

    let (mut command, sandbox) = builder
        .build(binary)
        .with_context(|| format!("failed to prepare sandbox for {}", binary.display()))?;
    command.args(args);
    if stdin_null {
        command.stdin(Stdio::null());
    }
    let child = command.spawn().with_context(|| {
        format!(
            "failed to spawn sandboxed {name} daemon binary {}",
            binary.display()
        )
    })?;
    Ok((child, Some(sandbox)))
}

fn spawn_reaper(
    name: &'static str,
    mut child: Child,
    exited: Arc<AtomicBool>,
    status_tx: SyncSender<Result<ExitStatus>>,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name(format!("capsa-reap-{name}"))
        .spawn(move || {
            let result = child
                .wait()
                .with_context(|| format!("failed waiting for {name} process"));
            exited.store(true, Ordering::Release);
            // Receiver may already have been dropped (e.g. if the
            // ChildHandle was dropped before the child exited). Fire
            // and forget in that case.
            let _ = status_tx.send(result);
        })
        .expect("spawning reaper thread should not fail")
}

fn sandbox_bypassed_by_env() -> bool {
    matches!(
        std::env::var("CAPSA_DISABLE_SANDBOX").as_deref(),
        Ok("1") | Ok("true") | Ok("yes") | Ok("on")
    )
}

/// Blocks until either `a` or `b` is reaped, returning the exit status
/// of whichever one finished first. The other handle stays alive and
/// its child is torn down implicitly when the caller drops the handle
/// (or explicitly via `shutdown()`).
pub(super) fn wait_either(a: &mut ChildHandle, b: &mut ChildHandle) -> Exited {
    loop {
        match a.status_rx.try_recv() {
            Ok(status) => return Exited::First(status),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                return Exited::First(Err(anyhow::anyhow!(
                    "{} reaper thread disconnected before reporting status",
                    a.name
                )));
            }
        }

        match b.status_rx.try_recv() {
            Ok(status) => return Exited::Second(status),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                return Exited::Second(Err(anyhow::anyhow!(
                    "{} reaper thread disconnected before reporting status",
                    b.name
                )));
            }
        }

        thread::sleep(DEFAULT_POLL_INTERVAL);
    }
}

/// Send SIGTERM, wait up to `shutdown_timeout` for the reaper to flip
/// `exited`, then escalate to SIGKILL if the child is still running.
fn kill_with_timeout(
    pid: i32,
    exited: &AtomicBool,
    shutdown_timeout: Duration,
    poll_interval: Duration,
) -> io::Result<()> {
    send_signal(pid, libc::SIGTERM)?;
    if wait_for_flag(exited, shutdown_timeout, poll_interval) {
        return Ok(());
    }

    send_signal(pid, libc::SIGKILL)?;
    if wait_for_flag(exited, shutdown_timeout, poll_interval) {
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        format!("process {pid} did not exit after SIGKILL"),
    ))
}

fn send_signal(pid: i32, sig: i32) -> io::Result<()> {
    // SAFETY: kill(2) with a valid pid is safe; the syscall reports
    // errors via errno rather than memory unsafety.
    let rc = unsafe { libc::kill(pid, sig) };
    if rc == 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    // ESRCH means the process is already gone, which is the happy
    // path for our use case (the reaper raced us to the finish line).
    if err.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }
    Err(err)
}

fn wait_for_flag(flag: &AtomicBool, timeout: Duration, interval: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if flag.load(Ordering::Acquire) {
            return true;
        }
        thread::sleep(interval);
    }
    flag.load(Ordering::Acquire)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::test_helpers::{
        env_lock, find_binary_on_path, unique_temp_path, EnvVarGuard,
    };
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    use std::time::Instant;

    fn write_executable_script(prefix: &str, body: &str) -> PathBuf {
        let path = unique_temp_path(prefix);
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(0o755)
            .open(&path)
            .expect("script should be writable");
        writeln!(file, "#!/bin/sh").unwrap();
        writeln!(file, "set -eu").unwrap();
        writeln!(file, "{body}").unwrap();
        path
    }

    fn bypass_builder() -> SandboxBuilder {
        capsa_sandbox::Sandbox::builder()
    }

    // ── resolve_binary ───────────────────────────────────────────────

    #[test]
    fn resolve_binary_honors_env_override() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let real = find_binary_on_path("true");
        let _guard = EnvVarGuard::set_path("CAPSA_TEST_BIN", &real);

        let resolved =
            resolve_binary("CAPSA_TEST_BIN", "true").expect("env override should resolve");
        assert_eq!(resolved, real);
    }

    #[test]
    fn resolve_binary_rejects_non_executable_env_override() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let non_exec = unique_temp_path("capsa-non-exec");
        std::fs::write(&non_exec, b"not executable").unwrap();
        let _guard = EnvVarGuard::set_path("CAPSA_TEST_BIN", &non_exec);

        let err = resolve_binary("CAPSA_TEST_BIN", "true")
            .expect_err("non-executable override should fail");

        let _ = std::fs::remove_file(non_exec);

        let rendered = format!("{err:#}");
        assert!(
            rendered.contains("not an executable file"),
            "unexpected: {rendered}"
        );
    }

    #[test]
    fn resolve_binary_falls_back_to_path_lookup() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _guard = EnvVarGuard::unset("CAPSA_TEST_NONE_BIN");

        let resolved = resolve_binary("CAPSA_TEST_NONE_BIN", "true")
            .expect("PATH fallback should resolve /bin/true");
        assert!(is_executable_file(&resolved));
        assert!(resolved.ends_with("true"));
    }

    // ── spawn_sandboxed (bypass path) ────────────────────────────────

    fn wait_for_exit(handle: &ChildHandle) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if handle.has_exited() {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("{} child did not exit in 5s", handle.name());
    }

    #[test]
    fn spawn_true_reports_success_via_shutdown() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");
        let binary = find_binary_on_path("true");

        let handle = spawn_sandboxed("test", &binary, bypass_builder(), &[], false)
            .expect("spawn should succeed");
        wait_for_exit(&handle);

        let status = handle.shutdown().expect("shutdown should succeed");
        assert!(status.success(), "expected success, got {status:?}");
    }

    #[test]
    fn spawn_false_reports_non_zero_exit_via_shutdown() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");
        let binary = find_binary_on_path("false");

        let handle = spawn_sandboxed("test", &binary, bypass_builder(), &[], false)
            .expect("spawn should succeed");
        wait_for_exit(&handle);

        let status = handle.shutdown().expect("shutdown should succeed");
        assert!(!status.success(), "expected failure, got {status:?}");
    }

    #[test]
    fn sandbox_bypass_env_accepts_truthy_matrix() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        for value in ["1", "true", "yes", "on"] {
            let _guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", value);
            assert!(
                sandbox_bypassed_by_env(),
                "value {value} should be treated as bypass"
            );
        }
        let _guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "no");
        assert!(!sandbox_bypassed_by_env(), "'no' should not bypass");
    }

    // ── shutdown escalation ──────────────────────────────────────────

    #[test]
    fn shutdown_escalates_to_sigkill_when_sigterm_ignored() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");
        let script = write_executable_script(
            "capsa-ignore-sigterm",
            "trap '' TERM\nwhile true; do sleep 1; done",
        );

        let mut handle = spawn_sandboxed("stubborn", &script, bypass_builder(), &[], false)
            .expect("spawn should succeed");
        // Shorten timeouts so the test does not block for 2 seconds.
        handle.shutdown_timeout = Duration::from_millis(250);
        handle.poll_interval = Duration::from_millis(10);

        let started = Instant::now();
        let status = handle
            .shutdown()
            .expect("shutdown should eventually succeed via SIGKILL");
        let elapsed = started.elapsed();

        let _ = std::fs::remove_file(&script);

        assert!(
            !status.success(),
            "SIGKILL'd process should not report success"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "shutdown should escalate within timeout + margin, took {elapsed:?}"
        );
    }

    // ── wait_either ──────────────────────────────────────────────────

    #[test]
    fn wait_either_returns_first_when_it_exits_first() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");
        let fast = find_binary_on_path("true");
        let slow = write_executable_script("capsa-slow", "sleep 5");

        let mut a = spawn_sandboxed("fast", &fast, bypass_builder(), &[], false).unwrap();
        let mut b = spawn_sandboxed("slow", &slow, bypass_builder(), &[], false).unwrap();

        match wait_either(&mut a, &mut b) {
            Exited::First(Ok(status)) => assert!(status.success()),
            other => panic!("expected First success, got {other:?}"),
        }

        // Cleanup: drop both handles, slow child gets SIGTERM.
        drop(a);
        drop(b);
        let _ = std::fs::remove_file(&slow);
    }

    #[test]
    fn wait_either_returns_second_when_it_exits_first() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");
        let fast = find_binary_on_path("true");
        let slow = write_executable_script("capsa-slow", "sleep 5");

        let mut a = spawn_sandboxed("slow", &slow, bypass_builder(), &[], false).unwrap();
        let mut b = spawn_sandboxed("fast", &fast, bypass_builder(), &[], false).unwrap();

        match wait_either(&mut a, &mut b) {
            Exited::Second(Ok(status)) => assert!(status.success()),
            other => panic!("expected Second success, got {other:?}"),
        }

        drop(a);
        drop(b);
        let _ = std::fs::remove_file(&slow);
    }

    #[test]
    fn wait_either_handles_already_exited_child() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");
        let fast = find_binary_on_path("true");
        let slow = write_executable_script("capsa-slow2", "sleep 5");

        let mut a = spawn_sandboxed("fast", &fast, bypass_builder(), &[], false).unwrap();
        let mut b = spawn_sandboxed("slow", &slow, bypass_builder(), &[], false).unwrap();

        // Give the fast child time to exit and the reaper to flip the
        // flag before calling wait_either. wait_either must still
        // report First rather than blocking forever on the slow one.
        while !a.has_exited() {
            std::thread::sleep(Duration::from_millis(5));
        }

        match wait_either(&mut a, &mut b) {
            Exited::First(Ok(_)) => {}
            other => panic!("expected First after pre-exit, got {other:?}"),
        }

        drop(a);
        drop(b);
        let _ = std::fs::remove_file(&slow);
    }

    // ── Drop path ────────────────────────────────────────────────────

    #[test]
    fn drop_does_not_signal_already_reaped_child() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");
        let binary = find_binary_on_path("true");

        let handle = spawn_sandboxed("test", &binary, bypass_builder(), &[], false).unwrap();

        // Wait for the reaper to mark the child as exited.
        while !handle.has_exited() {
            std::thread::sleep(Duration::from_millis(5));
        }

        // Drop should not send any signals and should not block on
        // kill_with_timeout. Most importantly, it must not panic.
        drop(handle);
    }

    impl std::fmt::Debug for Exited {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Exited::First(r) => write!(f, "First({r:?})"),
                Exited::Second(r) => write!(f, "Second({r:?})"),
            }
        }
    }
}
