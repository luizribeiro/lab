use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{ensure, Context, Result};

mod discover;

#[cfg(target_os = "macos")]
mod darwin;
#[cfg(target_os = "linux")]
mod linux;

#[cfg(feature = "tokio")]
pub mod tokio;

/// Cross-platform sandbox configuration.
#[derive(Debug, Clone, Default)]
pub struct SandboxSpec {
    /// Allow outbound/inbound networking from the sandboxed process.
    pub allow_network: bool,
    /// Allow access to the KVM hypervisor device (`/dev/kvm`) and the
    /// `KVM_*` ioctl set.
    ///
    /// Only meaningful on Linux, where it gates `/dev/kvm` read/write/ioctl
    /// grants and the libkrun-specific `KVM_*` ioctl allowlist. Callers that
    /// run a libkrun-based VMM should set this; other daemons should leave
    /// it `false`. No-op on other platforms.
    pub allow_kvm: bool,
    /// Allow access to the caller's controlling terminal (`/dev/tty`,
    /// `/dev/ttys*`) and the terminal-ioctl allowlist (`TCGETS*`,
    /// `TIOCGWINSZ`, `FIONREAD`, ...).
    ///
    /// Callers that expose an interactive console to the user (e.g. a
    /// libkrun VMM connecting the guest serial console to the host tty)
    /// should set this; non-interactive daemons should leave it `false`.
    /// Gates both the Linux seccomp ioctl allowlist and the macOS seatbelt
    /// `/dev/tty*` path grants.
    pub allow_interactive_tty: bool,
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

    pub fn allow_kvm(mut self, allow: bool) -> Self {
        self.allow_kvm = allow;
        self
    }

    pub fn allow_interactive_tty(mut self, allow: bool) -> Self {
        self.allow_interactive_tty = allow;
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
    /// Fails if the platform backend cannot satisfy the spec.
    #[cfg(target_os = "linux")]
    pub fn new(spec: SandboxSpec) -> Result<Self> {
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

/// Validates `fd_remaps` and installs a `pre_exec` callback on `command`
/// that remaps file descriptors into the child process accordingly. Use
/// this alongside [`Sandbox::command`] when the sandboxed child needs
/// specific fd numbers (for example, pre-opened sockets handed off from
/// a parent).
///
/// Validation is performed via [`validate_fd_remaps`] before the hook is
/// installed, so callers cannot accidentally skip it and let invalid
/// remaps surface as cryptic `dup2` failures inside the child's
/// `pre_exec` phase. Callers that want to fail earlier (e.g. while
/// building a spawn spec) may still invoke [`validate_fd_remaps`]
/// directly themselves.
pub fn configure_fd_remaps(command: &mut Command, fd_remaps: &[FdRemap]) -> Result<()> {
    validate_fd_remaps(fd_remaps)?;

    if fd_remaps.is_empty() {
        return Ok(());
    }

    use std::os::unix::process::CommandExt;

    let remaps = fd_remaps.to_vec();
    let child_hook = move || -> std::io::Result<()> {
        for remap in &remaps {
            // SAFETY: dup2 is async-signal-safe (POSIX.1) and takes two
            // integer fds. It atomically replaces target_fd with a
            // duplicate of source_fd, which is the intended effect.
            let rc = unsafe { libc::dup2(remap.source_fd, remap.target_fd) };
            if rc == -1 {
                return Err(std::io::Error::last_os_error());
            }

            if remap.source_fd != remap.target_fd {
                // SAFETY: close is async-signal-safe (POSIX.1). Passing an
                // invalid fd is not UB — the kernel returns EBADF. The
                // real hazard with `close` is closing an fd that another
                // part of the program still believes it owns, but we run
                // here in the freshly-forked child where the parent's
                // ownership is irrelevant: after exec, the parent's
                // `OwnedFd`s do not alias the child's fd table.
                let rc = unsafe { libc::close(remap.source_fd) };
                if rc == -1 {
                    return Err(std::io::Error::last_os_error());
                }
            }
        }
        Ok(())
    };

    // SAFETY: pre_exec runs `child_hook` in the child process after fork
    // and before exec, where only async-signal-safe work is permitted.
    // The closure's operations are all async-signal-safe:
    //   - iterating a pre-allocated `Vec<FdRemap>` by reference (pointer
    //     arithmetic over `Copy` i32 pairs; no allocation, no Drop),
    //   - `libc::dup2` and `libc::close` (POSIX async-signal-safe list),
    //   - `std::io::Error::last_os_error`, which reads `errno` and
    //     constructs an `io::Error` without allocating.
    // No locks are taken and no Rust destructors run on the happy path.
    unsafe {
        command.pre_exec(child_hook);
    }

    Ok(())
}

/// Returns whether `fd` currently refers to an open file description.
///
/// Encapsulates the only `unsafe` fcntl call in this module behind a safe
/// API. `F_GETFD` is a read-only query that takes a raw fd (an integer)
/// and cannot cause undefined behavior regardless of the value passed —
/// invalid fds yield `EBADF`, which we report as `Ok(false)`.
fn fd_is_open(fd: i32) -> std::io::Result<bool> {
    // SAFETY: `fcntl(fd, F_GETFD)` is a read-only kernel query on an
    // integer. It performs no memory access through `fd` and is safe to
    // call with any i32; an invalid fd simply returns -1 with errno
    // EBADF, which is handled below.
    let rc = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if rc == -1 {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EBADF) {
            return Ok(false);
        }
        return Err(err);
    }
    Ok(true)
}

/// Validates that `fd_remaps` can be applied to a spawn.
///
/// Returns an error if any source or target fd is negative, if there are
/// duplicates, if source and target fd sets overlap, or if a source fd is
/// not actually open. [`configure_fd_remaps`] calls this internally so
/// callers do not need to invoke it explicitly; it is exposed for
/// callers that want to validate remaps earlier than spawn time (e.g.
/// while building a spawn spec).
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

        match fd_is_open(remap.source_fd) {
            Ok(true) => {}
            Ok(false) => anyhow::bail!(
                "fd remap {index}: source_fd {} is not open",
                remap.source_fd
            ),
            Err(err) => {
                return Err(err).context(format!(
                    "fd remap {index}: failed to validate source_fd {}",
                    remap.source_fd
                ));
            }
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
    use std::os::fd::AsRawFd;
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
        let (read_end, mut write_end) = std::io::pipe().expect("failed to create pipe");
        writeln!(write_end, "ping").expect("failed to write to pipe");
        drop(write_end);

        let remaps = [FdRemap {
            source_fd: read_end.as_raw_fd(),
            target_fd: 101,
        }];

        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("IFS= read -r line <&101; [ \"$line\" = \"ping\" ]");
        super::configure_fd_remaps(&mut command, &remaps)
            .expect("expected configure_fd_remaps to succeed");

        let status = command.status().expect("failed to spawn child");
        assert!(status.success());
    }
}
