//! Generic child-process primitives used by `super::netd` and
//! `super::vmm`. Async-first: backed by `tokio::process::Child` via
//! lockin's `tokio_command` spawn path, so `wait` and `kill` are
//! awaitable without burning a dedicated reaper thread per child.
//!
//! Intentionally small and concrete: two daemon binaries do not
//! justify a trait hierarchy.
//!
//! The ownership story:
//!
//! - `spawn_sandboxed` builds a sandboxed `tokio::process::Command`
//!   (via lockin's `tokio_command` path) and spawns the child. The
//!   returned `ChildHandle` owns the resulting `SandboxedChild`, which
//!   in turn owns both the tokio `Child` and the `Sandbox` tmpdir.
//! - `kill_on_drop(true)` is set on every spawn so dropping the
//!   handle without an explicit teardown still SIGKILLs the child;
//!   tokio's orphan reaper keeps the zombie from lingering.
//! - `wait_by_ref` / `try_wait_by_ref` / `kill` delegate to
//!   `SandboxedChild`. Exit status is cached on the first successful
//!   wait so subsequent calls remain cheap.

use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use lockin::{Sandbox, SandboxBuilder};

pub(super) fn apply_syd_path(builder: SandboxBuilder) -> SandboxBuilder {
    match std::env::var_os("CAPSA_SYD_PATH") {
        Some(val) => builder.syd_path(std::path::PathBuf::from(val)),
        None => builder,
    }
}

pub(super) fn apply_library_dirs(mut builder: SandboxBuilder) -> SandboxBuilder {
    if let Some(val) = std::env::var_os("CAPSA_LIBRARY_DIRS") {
        for dir in std::env::split_paths(&val) {
            if !dir.as_os_str().is_empty() {
                builder = builder.exec_dir(dir);
            }
        }
    }
    builder
}

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

/// Handle to a spawned daemon child.
///
/// Created by [`spawn_sandboxed`]. The caller should either drive
/// it to completion via [`ChildHandle::wait_by_ref`], or explicitly
/// tear it down via [`ChildHandle::kill`] / [`ChildHandle::shutdown`].
/// Dropping the handle without waiting is also safe — `kill_on_drop`
/// is set on every spawned command so the child is SIGKILLed and
/// tokio's orphan reaper cleans up the zombie.
pub(super) struct ChildHandle {
    name: &'static str,
    pid: u32,
    child: tokio::process::Child,
    // Keeps the sandbox's private tmpdir alive for the lifetime of
    // the child. `None` in the `CAPSA_DISABLE_SANDBOX` bypass path.
    _sandbox: Option<Sandbox>,
    cached_status: Option<ExitStatus>,
    // Consulted only by the cfg(test) shutdown flow; set on every
    // spawn so production paths also see sane defaults if they ever
    // call into it.
    #[allow(dead_code)]
    shutdown_timeout: Duration,
    #[allow(dead_code)]
    poll_interval: Duration,
}

impl ChildHandle {
    #[cfg(test)]
    pub(super) fn name(&self) -> &'static str {
        self.name
    }

    /// Whether this handle has observed an exit status. Flips once
    /// [`wait_by_ref`](Self::wait_by_ref) returns or
    /// [`try_wait_by_ref`](Self::try_wait_by_ref) first yields `Some`.
    pub(super) fn has_exited(&self) -> bool {
        self.cached_status.is_some()
    }

    /// The child's OS process id.
    pub(super) fn pid(&self) -> u32 {
        self.pid
    }

    /// Await the child's exit on its own and return its status.
    /// Does **not** send any signals. Caches the first non-error
    /// result so subsequent calls are cheap.
    pub(super) async fn wait_by_ref(&mut self) -> Result<ExitStatus> {
        if let Some(status) = self.cached_status {
            return Ok(status);
        }
        let status = self
            .child
            .wait()
            .await
            .with_context(|| format!("failed waiting for {} process", self.name))?;
        self.cached_status = Some(status);
        Ok(status)
    }

    /// Non-blocking variant of [`wait_by_ref`]. Returns `Ok(None)` if
    /// the child is still running.
    pub(super) fn try_wait_by_ref(&mut self) -> Result<Option<ExitStatus>> {
        if let Some(status) = self.cached_status {
            return Ok(Some(status));
        }
        let status = self
            .child
            .try_wait()
            .with_context(|| format!("failed polling {} process", self.name))?;
        if let Some(s) = status {
            self.cached_status = Some(s);
        }
        Ok(status)
    }

    /// SIGKILL the child and await its reap. Safe to call after the
    /// child has already exited (becomes a no-op that just confirms
    /// the cached status).
    pub(super) async fn kill(&mut self) -> io::Result<()> {
        if self.cached_status.is_some() {
            return Ok(());
        }
        // start_kill is sync (just sends SIGKILL); wait is async and
        // performs the reap. Avoid the async tokio::process::Child::kill
        // helper because it takes &mut self twice in quick succession,
        // which composes awkwardly with our cached-status bookkeeping.
        match self.child.start_kill() {
            Ok(()) => {}
            Err(err) if err.raw_os_error() == Some(libc::ESRCH) => {}
            Err(err) => return Err(err),
        }
        match self.child.wait().await {
            Ok(status) => {
                self.cached_status = Some(status);
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    /// Explicit teardown: SIGTERM, wait up to `shutdown_timeout`, then
    /// escalate to SIGKILL, and return the final exit status.
    #[cfg(test)]
    pub(super) async fn shutdown(mut self) -> Result<ExitStatus> {
        if self.cached_status.is_none() {
            shutdown_with_timeout(
                &mut self.child,
                &mut self.cached_status,
                self.shutdown_timeout,
                self.poll_interval,
            )
            .await
            .with_context(|| format!("failed to shut down {} daemon", self.name))?;
        }
        self.cached_status
            .ok_or_else(|| anyhow::anyhow!("{} child produced no exit status", self.name))
    }
}

/// Spawns `binary` under a `lockin` sandbox (or bypassed via
/// `CAPSA_DISABLE_SANDBOX`) as a tokio child with `kill_on_drop`
/// set, and returns a [`ChildHandle`]. `fds` are file descriptors to
/// inherit into the child.
pub(super) fn spawn_sandboxed(
    name: &'static str,
    binary: &Path,
    builder: SandboxBuilder,
    fds: Vec<std::os::fd::OwnedFd>,
    args: &[String],
    stdin_null: bool,
) -> Result<ChildHandle> {
    let (child, sandbox) = build_and_spawn(name, binary, builder, fds, args, stdin_null)?;
    let pid = child
        .id()
        .with_context(|| format!("{name} child has no pid after spawn"))?;

    tracing::info!(daemon = name, pid, "spawned");

    Ok(ChildHandle {
        name,
        pid,
        child,
        _sandbox: sandbox,
        cached_status: None,
        shutdown_timeout: DEFAULT_SHUTDOWN_TIMEOUT,
        poll_interval: DEFAULT_POLL_INTERVAL,
    })
}

fn build_and_spawn(
    name: &'static str,
    binary: &Path,
    mut builder: SandboxBuilder,
    fds: Vec<std::os::fd::OwnedFd>,
    args: &[String],
    stdin_null: bool,
) -> Result<(tokio::process::Child, Option<Sandbox>)> {
    if sandbox_bypassed_by_env() {
        tracing::warn!(
            daemon = name,
            program = %binary.display(),
            "sandbox disabled via CAPSA_DISABLE_SANDBOX; running without sandbox"
        );
        let mut command = tokio::process::Command::new(binary);
        command.args(args);
        if stdin_null {
            command.stdin(Stdio::null());
        }
        command.kill_on_drop(true);
        use lockin_process::CommandFdExt;
        command.as_std_mut().seal_fds();
        for fd in fds {
            command.as_std_mut().keep_fd(fd);
        }
        let child = command.spawn().with_context(|| {
            format!("failed to spawn {name} daemon binary {}", binary.display())
        })?;
        return Ok((child, None));
    }

    for fd in fds {
        builder.inherit_fd(fd);
    }
    let mut sandbox_cmd = builder
        .tokio_command(binary)
        .with_context(|| format!("failed to prepare sandbox for {}", binary.display()))?;
    sandbox_cmd.args(args);
    if stdin_null {
        sandbox_cmd.stdin(Stdio::null());
    }
    sandbox_cmd.kill_on_drop(true);
    let sandbox_child = sandbox_cmd.spawn().with_context(|| {
        format!(
            "failed to spawn sandboxed {name} daemon binary {}",
            binary.display()
        )
    })?;
    let (child, sandbox) = sandbox_child.into_parts();
    Ok((child, Some(sandbox)))
}

/// Nix VM integration tests and CI environments where syd cannot run
/// inside the outer Nix sandbox set `CAPSA_DISABLE_SANDBOX=1` to skip
/// the sandbox wrapper.  The child runs without filesystem/network/
/// privilege restrictions in this mode.
fn sandbox_bypassed_by_env() -> bool {
    matches!(
        std::env::var("CAPSA_DISABLE_SANDBOX").as_deref(),
        Ok("1") | Ok("true") | Ok("yes") | Ok("on")
    )
}

/// SIGTERM, wait up to `shutdown_timeout` for the child to exit,
/// escalate to SIGKILL if still running, and record the final exit
/// status in `cached_status`.
#[cfg(test)]
async fn shutdown_with_timeout(
    child: &mut tokio::process::Child,
    cached_status: &mut Option<ExitStatus>,
    shutdown_timeout: Duration,
    poll_interval: Duration,
) -> io::Result<()> {
    send_signal(child, libc::SIGTERM)?;
    if let Some(status) = wait_up_to(child, shutdown_timeout, poll_interval).await? {
        *cached_status = Some(status);
        return Ok(());
    }

    send_signal(child, libc::SIGKILL)?;
    if let Some(status) = wait_up_to(child, shutdown_timeout, poll_interval).await? {
        *cached_status = Some(status);
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::TimedOut,
        "process did not exit after SIGKILL",
    ))
}

#[cfg(test)]
fn send_signal(child: &tokio::process::Child, sig: i32) -> io::Result<()> {
    let Some(pid) = child.id() else {
        // Already reaped.
        return Ok(());
    };
    // SAFETY: kill(2) with a valid pid is safe; the syscall reports
    // errors via errno rather than memory unsafety.
    let rc = unsafe { libc::kill(pid as i32, sig) };
    if rc == 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    // ESRCH means the process is already gone, which is fine.
    if err.raw_os_error() == Some(libc::ESRCH) {
        return Ok(());
    }
    Err(err)
}

#[cfg(test)]
async fn wait_up_to(
    child: &mut tokio::process::Child,
    timeout: Duration,
    poll_interval: Duration,
) -> io::Result<Option<ExitStatus>> {
    let deadline = tokio::time::Instant::now() + timeout;
    while tokio::time::Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        tokio::time::sleep(poll_interval).await;
    }
    Ok(child.try_wait()?)
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

    // Shell scripts don't need library paths — /bin/sh is always
    // accessible to the sandbox without explicit grants.
    fn bypass_builder() -> SandboxBuilder {
        lockin::Sandbox::builder()
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

    async fn wait_for_exit(handle: &mut ChildHandle) {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while tokio::time::Instant::now() < deadline {
            if handle.try_wait_by_ref().expect("try_wait_by_ref").is_some() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("{} child did not exit in 5s", handle.name());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn spawn_true_reports_success_via_shutdown() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");
        let binary = find_binary_on_path("true");

        let mut handle = spawn_sandboxed("test", &binary, bypass_builder(), vec![], &[], false)
            .expect("spawn should succeed");
        wait_for_exit(&mut handle).await;

        let status = handle.shutdown().await.expect("shutdown should succeed");
        assert!(status.success(), "expected success, got {status:?}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn spawn_false_reports_non_zero_exit_via_shutdown() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");
        let binary = find_binary_on_path("false");

        let mut handle = spawn_sandboxed("test", &binary, bypass_builder(), vec![], &[], false)
            .expect("spawn should succeed");
        wait_for_exit(&mut handle).await;

        let status = handle.shutdown().await.expect("shutdown should succeed");
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

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_escalates_to_sigkill_when_sigterm_ignored() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");
        let script = write_executable_script(
            "capsa-ignore-sigterm",
            "trap '' TERM\nwhile true; do sleep 1; done",
        );

        let mut handle = spawn_sandboxed("stubborn", &script, bypass_builder(), vec![], &[], false)
            .expect("spawn should succeed");
        // Shorten timeouts so the test does not block for 2 seconds.
        handle.shutdown_timeout = Duration::from_millis(250);
        handle.poll_interval = Duration::from_millis(10);

        let started = Instant::now();
        let status = handle
            .shutdown()
            .await
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

    // ── Drop path ────────────────────────────────────────────────────

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn drop_does_not_panic_on_already_reaped_child() {
        let _lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");
        let binary = find_binary_on_path("true");

        let mut handle =
            spawn_sandboxed("test", &binary, bypass_builder(), vec![], &[], false).unwrap();

        // Drive to exit.
        let status = handle
            .wait_by_ref()
            .await
            .expect("wait_by_ref should succeed");
        assert!(status.success());

        // Drop on a handle whose cached status is already set must
        // not panic or block.
        drop(handle);
    }
}
