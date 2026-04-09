use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{ensure, Context, Result};

mod discover;

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

/// A prepared sandbox environment: holds the private tmp directory and any
/// per-platform state needed to build sandboxed commands.
///
/// Use [`Sandbox::new`] to construct one from a [`SandboxSpec`], then call
/// [`Sandbox::command`] to mint a [`std::process::Command`] that runs a given
/// program inside this sandbox. The caller must keep the `Sandbox` alive until
/// the spawned child exits — dropping it earlier removes the private tmp
/// directory out from under the child.
///
/// ```no_run
/// use std::path::Path;
/// use capsa_sandbox::{Sandbox, SandboxSpec};
///
/// let spec = SandboxSpec::new();
/// let sandbox = Sandbox::new(spec).unwrap();
/// let mut child = sandbox
///     .command(Path::new("/bin/true"))
///     .spawn()
///     .unwrap();
/// child.wait().unwrap();
/// ```
pub struct Sandbox {
    spec: SandboxSpec,
    private_tmp: tempfile::TempDir,
    #[cfg(target_os = "linux")]
    syd: PathBuf,
}

impl Sandbox {
    /// Prepares a sandbox from `spec`.
    ///
    /// Fails if the platform backend cannot satisfy the spec. On Linux this
    /// includes `spec.allow_network == true`, because the `syd` backend's
    /// network mediation currently conflicts with capsa-netd.
    #[cfg(target_os = "linux")]
    pub fn new(spec: SandboxSpec) -> Result<Self> {
        if spec.allow_network {
            anyhow::bail!(
                "capsa-sandbox: the Linux syd backend cannot currently sandbox processes that require \
                 network access (syd's net mediation conflicts with capsa-netd). Callers that need \
                 network access should either run the program without a sandbox or wait for the \
                 backend to gain netd-compatible network mediation."
            );
        }

        let syd = linux::find_in_path("syd").ok_or_else(|| {
            anyhow::anyhow!(
                "Linux sandbox requires `syd` on PATH. Install it (e.g. via `nix develop`)"
            )
        })?;

        let private_tmp = create_private_tmp()?;

        Ok(Self {
            spec,
            private_tmp,
            syd,
        })
    }

    #[cfg(target_os = "macos")]
    pub fn new(spec: SandboxSpec) -> Result<Self> {
        let private_tmp = create_private_tmp()?;

        Ok(Self { spec, private_tmp })
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    pub fn new(_spec: SandboxSpec) -> Result<Self> {
        anyhow::bail!("capsa-sandbox: sandboxing is not implemented for this platform")
    }

    /// Returns the spec used to build this sandbox.
    pub fn spec(&self) -> &SandboxSpec {
        &self.spec
    }

    /// Path of the private tmp directory that the sandbox will expose to its
    /// children as `$TMPDIR`.
    pub fn private_tmp(&self) -> &Path {
        self.private_tmp.path()
    }

    /// Builds a [`std::process::Command`] that runs `program` inside this
    /// sandbox. The caller can configure `args`, `env`, `stdin`/`stdout`/
    /// `stderr`, `current_dir` on the returned command as usual before calling
    /// `spawn()`.
    ///
    /// `TMPDIR`/`TMP`/`TEMP` are pre-set to the sandbox's private tmp
    /// directory; the caller may override them if desired.
    #[cfg(target_os = "linux")]
    pub fn command(&self, program: &Path) -> Command {
        linux::build_sandbox_command(&self.spec, self.private_tmp.path(), &self.syd, program)
    }

    #[cfg(target_os = "macos")]
    pub fn command(&self, program: &Path) -> Command {
        darwin::build_sandbox_command(&self.spec, self.private_tmp.path(), program)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    pub fn command(&self, _program: &Path) -> Command {
        unreachable!("Sandbox::new fails on unsupported platforms")
    }
}

fn create_private_tmp() -> Result<tempfile::TempDir> {
    let base = std::env::temp_dir().join("capsa-sandbox");
    std::fs::create_dir_all(&base)
        .with_context(|| format!("failed to create sandbox temp base {}", base.display()))?;

    tempfile::Builder::new()
        .prefix("sbx-")
        .tempdir_in(&base)
        .with_context(|| format!("failed to create private temp dir in {}", base.display()))
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

/// Installs a `pre_exec` callback on `command` that remaps file descriptors
/// into the child process according to `fd_remaps`. Use this alongside
/// [`Sandbox::command`] when the sandboxed child needs specific fd numbers
/// (for example, pre-opened sockets handed off from a parent).
pub fn configure_fd_remaps(command: &mut Command, fd_remaps: &[FdRemap]) {
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

/// Validates that `fd_remaps` can be applied to a spawn.
///
/// Returns an error if any source or target fd is negative, if there are
/// duplicates, if source and target fd sets overlap, or if a source fd is
/// not actually open. Call this before [`configure_fd_remaps`] to catch
/// invalid remaps with a clear error, rather than letting them fail inside
/// the child's `pre_exec` hook.
pub fn validate_fd_remaps(fd_remaps: &[FdRemap]) -> Result<()> {
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

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::process::Command;

    use super::{validate_fd_remaps, FdRemap};

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

        let remaps = [FdRemap {
            source_fd: read_end.as_raw_fd(),
            target_fd: 101,
        }];
        validate_fd_remaps(&remaps).expect("expected validation to pass");

        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("IFS= read -r line <&101; [ \"$line\" = \"ping\" ]");
        super::configure_fd_remaps(&mut command, &remaps);

        let status = command.status().expect("failed to spawn child");
        assert!(status.success());
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
