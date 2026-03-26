use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};

use anyhow::{ensure, Context, Result};

#[cfg(target_os = "macos")]
mod darwin;
#[cfg(target_os = "linux")]
mod linux;

/// Cross-platform sandbox configuration.
#[derive(Debug, Clone, Default)]
pub struct SandboxSpec {
    /// Allow outbound/inbound networking from the sandboxed process.
    pub allow_network: bool,
    /// Paths that should be readable from inside the sandbox.
    pub read_only_paths: Vec<PathBuf>,
    /// Paths that should be writable from inside the sandbox.
    pub read_write_paths: Vec<PathBuf>,
    /// Paths that should be allowed to perform ioctl operations.
    ///
    /// Backends may apply this with different precision depending on platform
    /// sandbox capabilities.
    pub ioctl_paths: Vec<PathBuf>,
}

impl SandboxSpec {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn allow_network(mut self, allow: bool) -> Self {
        self.allow_network = allow;
        self
    }
}

/// An fd to inherit into the child process at a specific target fd number.
///
/// Each remap must use unique source and target fd numbers, and source/target
/// fd sets must not overlap across remaps. This keeps remapping deterministic
/// in the child pre-exec phase.
#[derive(Debug, Clone)]
pub struct FdRemap {
    /// Source fd in the parent process.
    pub source_fd: i32,
    /// Target fd number in the child process.
    pub target_fd: i32,
}

pub struct SandboxedChild {
    child: Child,
    cleanup_paths: Option<Vec<PathBuf>>,
}

impl SandboxedChild {
    pub(crate) fn new(child: Child, cleanup_paths: Vec<PathBuf>) -> Self {
        Self {
            child,
            cleanup_paths: Some(cleanup_paths),
        }
    }

    pub fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        let status = self.child.try_wait()?;
        if status.is_some() {
            self.cleanup_now();
        }
        Ok(status)
    }

    /// Sends a kill signal to the child process.
    ///
    /// This does not reap the child. Call `wait_blocking()` (or `try_wait()` until
    /// it returns `Some`) to avoid leaving a zombie process.
    pub fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill()
    }

    pub fn wait_blocking(&mut self) -> std::io::Result<ExitStatus> {
        let status = self.child.wait()?;
        self.cleanup_now();
        Ok(status)
    }

    pub fn wait(mut self) -> std::io::Result<ExitStatus> {
        self.wait_blocking()
    }

    fn cleanup_now(&mut self) {
        let Some(paths) = self.cleanup_paths.take() else {
            return;
        };

        for path in paths {
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

impl Drop for SandboxedChild {
    fn drop(&mut self) {
        let _ = self.child.try_wait();
        self.cleanup_now();
    }
}

/// Spawn `program` with `args` inside the platform sandbox.
///
/// - macOS: seatbelt profile via `sandbox-exec`
/// - Linux: `syd` backend (fail-closed by default)
///
/// Set `CAPSA_DISABLE_SANDBOX=1` (or `true`/`yes`/`on`) to bypass sandboxing.
#[cfg(target_os = "macos")]
pub fn spawn_sandboxed(
    program: &Path,
    args: &[String],
    spec: &SandboxSpec,
) -> Result<SandboxedChild> {
    spawn_sandboxed_with_fds(program, args, spec, &[])
}

#[cfg(target_os = "linux")]
pub fn spawn_sandboxed(
    program: &Path,
    args: &[String],
    spec: &SandboxSpec,
) -> Result<SandboxedChild> {
    spawn_sandboxed_with_fds(program, args, spec, &[])
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn spawn_sandboxed(
    _program: &Path,
    _args: &[String],
    _spec: &SandboxSpec,
) -> Result<SandboxedChild> {
    anyhow::bail!("sandboxing is not implemented for this platform")
}

#[cfg(target_os = "macos")]
pub fn spawn_sandboxed_with_fds(
    program: &Path,
    args: &[String],
    spec: &SandboxSpec,
    fd_remaps: &[FdRemap],
) -> Result<SandboxedChild> {
    validate_fd_remaps(fd_remaps)?;

    if sandbox_disabled() {
        eprintln!("warning: sandbox disabled via CAPSA_DISABLE_SANDBOX; running without sandbox");
        return spawn_direct_with_fds(program, args, fd_remaps);
    }

    darwin::spawn_with_sandbox_exec(program, args, spec, fd_remaps)
}

#[cfg(target_os = "linux")]
pub fn spawn_sandboxed_with_fds(
    program: &Path,
    args: &[String],
    spec: &SandboxSpec,
    fd_remaps: &[FdRemap],
) -> Result<SandboxedChild> {
    validate_fd_remaps(fd_remaps)?;

    if sandbox_disabled() {
        eprintln!("warning: sandbox disabled via CAPSA_DISABLE_SANDBOX; running without sandbox");
        return spawn_direct_with_fds(program, args, fd_remaps);
    }

    linux::spawn_with_syd(program, args, spec, fd_remaps)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn spawn_sandboxed_with_fds(
    _program: &Path,
    _args: &[String],
    _spec: &SandboxSpec,
    _fd_remaps: &[FdRemap],
) -> Result<SandboxedChild> {
    anyhow::bail!("sandboxing is not implemented for this platform")
}

pub(crate) fn configure_fd_remaps(command: &mut Command, fd_remaps: &[FdRemap]) {
    if fd_remaps.is_empty() {
        return;
    }

    use std::os::unix::process::CommandExt;

    let remaps = fd_remaps.to_vec();
    // SAFETY: pre_exec runs in the child process after fork and before exec.
    // We only call async-signal-safe libc operations here (dup2/close).
    unsafe {
        command.pre_exec(move || {
            for remap in &remaps {
                if libc::dup2(remap.source_fd, remap.target_fd) == -1 {
                    return Err(std::io::Error::last_os_error());
                }

                if remap.source_fd != remap.target_fd && libc::close(remap.source_fd) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
            }

            Ok(())
        });
    }
}

fn validate_fd_remaps(fd_remaps: &[FdRemap]) -> Result<()> {
    let mut seen_sources = std::collections::HashSet::new();
    let mut seen_targets = std::collections::HashSet::new();

    for (index, remap) in fd_remaps.iter().enumerate() {
        ensure!(
            remap.source_fd >= 0,
            "fd remap {index}: source_fd must be >= 0 (got {})",
            remap.source_fd
        );
        ensure!(
            remap.target_fd >= 0,
            "fd remap {index}: target_fd must be >= 0 (got {})",
            remap.target_fd
        );
        ensure!(
            seen_sources.insert(remap.source_fd),
            "fd remap {index}: duplicate source_fd {}",
            remap.source_fd
        );
        ensure!(
            seen_targets.insert(remap.target_fd),
            "fd remap {index}: duplicate target_fd {}",
            remap.target_fd
        );

        // SAFETY: fcntl(F_GETFD) doesn't mutate memory and is used here only
        // to validate that the fd currently refers to an open file description.
        let rc = unsafe { libc::fcntl(remap.source_fd, libc::F_GETFD) };
        if rc == -1 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::EBADF) {
                anyhow::bail!(
                    "fd remap {index}: source_fd {} is not open",
                    remap.source_fd
                );
            }
            return Err(err).context(format!(
                "fd remap {index}: failed to validate source_fd {}",
                remap.source_fd
            ));
        }
    }

    for source_fd in &seen_sources {
        ensure!(
            !seen_targets.contains(source_fd),
            "fd remap: overlapping source/target fd {} is not supported",
            source_fd
        );
    }

    Ok(())
}

fn sandbox_disabled() -> bool {
    matches!(
        std::env::var("CAPSA_DISABLE_SANDBOX").as_deref(),
        Ok("1") | Ok("true") | Ok("yes") | Ok("on")
    )
}

fn spawn_direct_with_fds(
    program: &Path,
    args: &[String],
    fd_remaps: &[FdRemap],
) -> Result<SandboxedChild> {
    let mut command = Command::new(program);
    command.args(args);
    configure_fd_remaps(&mut command, fd_remaps);

    let child = command
        .spawn()
        .with_context(|| format!("failed to spawn {}", program.display()))?;

    Ok(SandboxedChild::new(child, vec![]))
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::path::Path;
    use std::process::Command;

    use super::{spawn_direct_with_fds, validate_fd_remaps, FdRemap};

    #[test]
    fn invalid_source_fd_is_rejected() {
        let remaps = [FdRemap {
            source_fd: -1,
            target_fd: 42,
        }];

        let err = validate_fd_remaps(&remaps).expect_err("expected validation to fail");
        assert!(err.to_string().contains("source_fd must be >= 0"));
    }

    #[test]
    fn duplicate_target_fd_is_rejected() {
        let source_a = tempfile::tempfile().expect("failed to open temp file a");
        let source_b = tempfile::tempfile().expect("failed to open temp file b");
        let remaps = [
            FdRemap {
                source_fd: source_a.as_raw_fd(),
                target_fd: 42,
            },
            FdRemap {
                source_fd: source_b.as_raw_fd(),
                target_fd: 42,
            },
        ];

        let err = validate_fd_remaps(&remaps).expect_err("expected validation to fail");
        assert!(err.to_string().contains("duplicate target_fd 42"));
    }

    #[test]
    fn duplicate_source_fd_is_rejected() {
        let file = tempfile::tempfile().expect("failed to open temp file");
        let source_fd = file.as_raw_fd();
        let remaps = [
            FdRemap {
                source_fd,
                target_fd: 45,
            },
            FdRemap {
                source_fd,
                target_fd: 46,
            },
        ];

        let err = validate_fd_remaps(&remaps).expect_err("expected validation to fail");
        assert!(err.to_string().contains("duplicate source_fd"));
    }

    #[test]
    fn overlapping_source_and_target_fds_are_rejected() {
        let file_a = tempfile::tempfile().expect("failed to open temp file a");
        let file_b = tempfile::tempfile().expect("failed to open temp file b");
        let source_a = file_a.as_raw_fd();
        let source_b = file_b.as_raw_fd();

        let remaps = [
            FdRemap {
                source_fd: source_a,
                target_fd: source_b,
            },
            FdRemap {
                source_fd: source_b,
                target_fd: 47,
            },
        ];

        let err = validate_fd_remaps(&remaps).expect_err("expected validation to fail");
        assert!(err.to_string().contains("overlapping source/target fd"));
    }

    #[test]
    fn closed_source_fd_is_rejected() {
        let source_fd = {
            let file = tempfile::tempfile().expect("failed to open temp file");
            file.as_raw_fd()
        };

        let remaps = [FdRemap {
            source_fd,
            target_fd: 43,
        }];

        let err = validate_fd_remaps(&remaps).expect_err("expected validation to fail");
        assert!(err.to_string().contains("is not open"));
    }

    #[test]
    fn valid_remap_passes_validation() {
        let file = tempfile::tempfile().expect("failed to open temp file");
        let remaps = [FdRemap {
            source_fd: file.as_raw_fd(),
            target_fd: 44,
        }];

        validate_fd_remaps(&remaps).expect("expected validation to pass");
    }

    #[test]
    fn remapped_fd_is_accessible_in_child_process() {
        let (read_end, mut write_end) = create_pipe();
        writeln!(write_end, "ping").expect("failed to write to pipe");
        drop(write_end);

        let args = vec![
            "-c".to_string(),
            "IFS= read -r line <&101; [ \"$line\" = \"ping\" ]".to_string(),
        ];

        let child = spawn_direct_with_fds(
            Path::new("/bin/sh"),
            &args,
            &[FdRemap {
                source_fd: read_end.as_raw_fd(),
                target_fd: 101,
            }],
        )
        .expect("failed to spawn child");

        let status = child.wait().expect("failed waiting for child");
        assert!(status.success());
    }

    #[test]
    fn try_wait_returns_none_while_child_running() {
        let mut child = spawn_sleep_child(5, vec![]);

        let status = child.try_wait().expect("try_wait should succeed");
        assert!(status.is_none(), "child should still be running");

        child.kill().expect("kill should succeed");
        let _ = child.wait_blocking().expect("wait_blocking should succeed");
    }

    #[test]
    fn try_wait_returns_some_and_cleans_up_paths() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        let cleanup_file = temp.path().join("cleanup-on-try-wait");
        std::fs::write(&cleanup_file, b"x").expect("failed to create cleanup file");

        let mut child = spawn_quick_exit_child(vec![cleanup_file.clone()]);

        let status = loop {
            if let Some(status) = child.try_wait().expect("try_wait should succeed") {
                break status;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        };

        assert!(status.success(), "child should exit successfully");
        assert!(
            !cleanup_file.exists(),
            "cleanup file should be removed once try_wait observes exit"
        );
    }

    #[test]
    fn kill_then_wait_blocking_reaps_child() {
        let mut child = spawn_sleep_child(30, vec![]);

        child.kill().expect("kill should succeed");
        let status = child
            .wait_blocking()
            .expect("wait_blocking should reap killed child");
        assert!(
            !status.success(),
            "killed child should not exit successfully"
        );
    }

    #[test]
    fn cleanup_paths_are_removed_after_wait_blocking() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");
        let cleanup_file = temp.path().join("cleanup.file");
        let cleanup_dir = temp.path().join("cleanup-dir");

        std::fs::write(&cleanup_file, b"x").expect("failed to create cleanup file");
        std::fs::create_dir_all(&cleanup_dir).expect("failed to create cleanup dir");

        let mut child = spawn_quick_exit_child(vec![cleanup_file.clone(), cleanup_dir.clone()]);
        let status = child.wait_blocking().expect("wait_blocking should succeed");
        assert!(status.success(), "child should exit successfully");

        assert!(!cleanup_file.exists(), "cleanup file should be removed");
        assert!(!cleanup_dir.exists(), "cleanup dir should be removed");
    }

    #[test]
    fn cleanup_paths_are_removed_on_drop_after_try_wait_or_kill() {
        let temp = tempfile::tempdir().expect("failed to create temp dir");

        let try_wait_path = temp.path().join("cleanup-after-try-wait");
        std::fs::write(&try_wait_path, b"x").expect("failed to create cleanup file");
        let try_wait_pid = {
            let mut child = spawn_sleep_child(30, vec![try_wait_path.clone()]);
            let status = child.try_wait().expect("try_wait should succeed");
            assert!(status.is_none(), "child should still be running");
            let pid = child.child.id() as libc::pid_t;
            drop(child);
            pid
        };
        assert!(
            !try_wait_path.exists(),
            "cleanup file should be removed when child is dropped after try_wait"
        );

        // SAFETY: pid comes from a child process spawned in this test process.
        let kill_rc = unsafe { libc::kill(try_wait_pid, libc::SIGKILL) };
        assert_eq!(
            kill_rc, 0,
            "kill should succeed after dropping child handle"
        );
        // SAFETY: pid comes from a child process spawned in this test process.
        let wait_rc = unsafe { libc::waitpid(try_wait_pid, std::ptr::null_mut(), 0) };
        assert_eq!(
            wait_rc, try_wait_pid,
            "waitpid should reap child dropped after try_wait"
        );

        let kill_path = temp.path().join("cleanup-after-kill");
        std::fs::write(&kill_path, b"x").expect("failed to create cleanup file");
        let pid = {
            let mut child = spawn_sleep_child(30, vec![kill_path.clone()]);
            child.kill().expect("kill should succeed");
            let pid = child.child.id() as libc::pid_t;
            drop(child);
            pid
        };

        assert!(
            !kill_path.exists(),
            "cleanup file should be removed when child is dropped after kill"
        );

        // SAFETY: pid comes from a child process spawned in this test process.
        let rc = unsafe { libc::waitpid(pid, std::ptr::null_mut(), 0) };
        assert_eq!(rc, pid, "waitpid should reap killed child after drop");
    }

    fn spawn_sleep_child(
        seconds: u64,
        cleanup_paths: Vec<std::path::PathBuf>,
    ) -> super::SandboxedChild {
        let child = Command::new("/bin/sh")
            .arg("-c")
            .arg(format!("sleep {seconds}"))
            .spawn()
            .expect("failed to spawn sleep child");

        super::SandboxedChild::new(child, cleanup_paths)
    }

    fn spawn_quick_exit_child(cleanup_paths: Vec<std::path::PathBuf>) -> super::SandboxedChild {
        let child = Command::new("/bin/sh")
            .arg("-c")
            .arg("exit 0")
            .spawn()
            .expect("failed to spawn quick-exit child");

        super::SandboxedChild::new(child, cleanup_paths)
    }

    fn create_pipe() -> (OwnedFd, std::fs::File) {
        let mut raw_fds = [0; 2];
        // SAFETY: `pipe` initializes both elements of raw_fds on success.
        let rc = unsafe { libc::pipe(raw_fds.as_mut_ptr()) };
        assert_eq!(rc, 0, "failed to create pipe");

        // SAFETY: raw_fds[0] is a valid owned fd from pipe.
        let read_end = unsafe { OwnedFd::from_raw_fd(raw_fds[0]) };
        // SAFETY: raw_fds[1] is a valid owned fd from pipe.
        let write_end = unsafe { std::fs::File::from_raw_fd(raw_fds[1]) };

        (read_end, write_end)
    }
}
