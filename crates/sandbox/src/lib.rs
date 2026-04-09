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

/// An owned source fd to inherit into the child process at a specific
/// target fd number.
///
/// Ownership of `source` is moved into the remap and, ultimately, into the
/// `pre_exec` closure installed on the child [`Command`]. This keeps the
/// source fd alive in the parent from construction until after the child
/// has run its `dup2` hook — closing the TOCTOU window that a raw fd
/// number would expose — and lets the type system guarantee that `source`
/// is open, non-negative, and unique across remaps (since `OwnedFd` is
/// non-`Copy` and represents exclusive ownership of an fd).
#[derive(Debug)]
pub struct FdRemap {
    /// Source fd owned by this remap; duplicated into the child at
    /// `target_fd`.
    pub source: std::os::fd::OwnedFd,
    /// Target fd number in the child process.
    pub target_fd: std::os::fd::RawFd,
}

/// Validates `fd_remaps` and installs a `pre_exec` callback on `command`
/// that remaps file descriptors into the child process accordingly. Takes
/// the remaps by value: each [`FdRemap`]'s `OwnedFd` is moved into the
/// child-side hook, which (a) keeps the source fds open in the parent
/// until after spawn, (b) closes them in the child via explicit `close`
/// after the `dup2`, and (c) closes them in the parent when the
/// [`Command`] (and with it this closure) is dropped.
///
/// Validation is performed via [`validate_fd_remaps`] before the hook is
/// installed; callers that want to fail earlier (e.g. while building a
/// spawn spec) may still invoke [`validate_fd_remaps`] directly
/// themselves.
pub fn configure_fd_remaps(command: &mut Command, fd_remaps: Vec<FdRemap>) -> Result<()> {
    validate_fd_remaps(&fd_remaps)?;

    if fd_remaps.is_empty() {
        return Ok(());
    }

    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;

    let child_hook = move || -> std::io::Result<()> {
        for remap in &fd_remaps {
            let source = remap.source.as_raw_fd();
            let target = remap.target_fd;

            // SAFETY: dup2 is async-signal-safe (POSIX.1). `source` is a
            // valid fd owned by this remap's `OwnedFd`; `target` is an
            // integer and cannot cause UB regardless of value. dup2
            // atomically replaces `target` with a duplicate of `source`
            // in the child's fd table, which is the intended effect.
            let rc = unsafe { libc::dup2(source, target) };
            if rc == -1 {
                return Err(std::io::Error::last_os_error());
            }

            if source != target {
                // SAFETY: close is async-signal-safe (POSIX.1). We close
                // the child-side `source` explicitly so it does not leak
                // past `exec`; the parent's `OwnedFd` copy remains alive
                // and will be closed when the `Command` (and with it
                // this closure) is dropped in the parent.
                let rc = unsafe { libc::close(source) };
                if rc == -1 {
                    return Err(std::io::Error::last_os_error());
                }
            }
        }
        Ok(())
    };

    // SAFETY: pre_exec runs `child_hook` in the child after fork and
    // before exec, where only async-signal-safe work is permitted. The
    // closure's operations are all async-signal-safe:
    //   - iterating a pre-allocated `Vec<FdRemap>` by reference (no
    //     allocation; the only `Drop`-bearing field is `OwnedFd`, which
    //     is not dropped during iteration),
    //   - `OwnedFd::as_raw_fd` (reads an inline integer),
    //   - `libc::dup2` and `libc::close` (POSIX async-signal-safe list),
    //   - `std::io::Error::last_os_error`, which in the current std
    //     implementation just reads `errno` and stores it in an inline
    //     `Os` variant (no allocation, no locks). This is an empirical
    //     claim about current std; the API does not formally promise
    //     async-signal safety.
    // No Rust destructors run on the happy path: on success the child
    // falls through to `exec`, discarding the Rust stack; on failure the
    // error is returned to std's spawn machinery which reports it and
    // calls `_exit`.
    unsafe {
        command.pre_exec(child_hook);
    }

    Ok(())
}

/// Validates that `fd_remaps` can be applied to a spawn.
///
/// Checks that each `target_fd` is non-negative, that no two remaps share
/// the same `target_fd`, and that no `target_fd` overlaps another remap's
/// source fd (which would silently clobber that source during ordered
/// processing). Source fds themselves are [`OwnedFd`]s, so the type
/// system already guarantees they are open, non-negative, and unique
/// across remaps — there is nothing left to check on the source side.
///
/// [`configure_fd_remaps`] calls this internally; callers do not need to
/// invoke it explicitly unless they want to fail earlier than spawn time.
pub fn validate_fd_remaps(fd_remaps: &[FdRemap]) -> Result<()> {
    use std::collections::HashSet;
    use std::os::fd::AsRawFd;

    let sources: HashSet<std::os::fd::RawFd> =
        fd_remaps.iter().map(|r| r.source.as_raw_fd()).collect();
    let mut seen_targets = HashSet::new();

    for (index, remap) in fd_remaps.iter().enumerate() {
        ensure!(
            remap.target_fd >= 0,
            "fd remap {index}: target_fd must be >= 0 (got {})",
            remap.target_fd
        );
        ensure!(
            seen_targets.insert(remap.target_fd),
            "fd remap {index}: duplicate target_fd {}",
            remap.target_fd
        );
        ensure!(
            !sources.contains(&remap.target_fd),
            "fd remap {index}: target_fd {} overlaps another remap's source fd",
            remap.target_fd
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::os::fd::{AsRawFd, OwnedFd};
    use std::process::Command;

    use super::{validate_fd_remaps, FdRemap};

    fn owned_tempfile() -> OwnedFd {
        tempfile::tempfile()
            .expect("failed to open temp file")
            .into()
    }

    #[test]
    fn negative_target_fd_is_rejected() {
        let remaps = vec![FdRemap {
            source: owned_tempfile(),
            target_fd: -1,
        }];

        let err = validate_fd_remaps(&remaps).expect_err("expected validation to fail");
        assert!(err.to_string().contains("target_fd must be >= 0"));
    }

    #[test]
    fn duplicate_target_fd_is_rejected() {
        let remaps = vec![
            FdRemap {
                source: owned_tempfile(),
                target_fd: 42,
            },
            FdRemap {
                source: owned_tempfile(),
                target_fd: 42,
            },
        ];

        let err = validate_fd_remaps(&remaps).expect_err("expected validation to fail");
        assert!(err.to_string().contains("duplicate target_fd 42"));
    }

    #[test]
    fn target_overlapping_another_source_is_rejected() {
        let source_a = owned_tempfile();
        let source_b = owned_tempfile();
        let source_b_raw = source_b.as_raw_fd();

        // The first remap targets source_b's raw fd, which would silently
        // clobber source_b before the second remap could read from it.
        let remaps = vec![
            FdRemap {
                source: source_a,
                target_fd: source_b_raw,
            },
            FdRemap {
                source: source_b,
                target_fd: 200,
            },
        ];

        let err = validate_fd_remaps(&remaps).expect_err("expected validation to fail");
        assert!(
            err.to_string()
                .contains("overlaps another remap's source fd"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn valid_remaps_pass_validation() {
        let remaps = vec![
            FdRemap {
                source: owned_tempfile(),
                target_fd: 44,
            },
            FdRemap {
                source: owned_tempfile(),
                target_fd: 45,
            },
        ];

        validate_fd_remaps(&remaps).expect("expected validation to pass");
    }

    #[test]
    fn remapped_fd_is_accessible_in_child_process() {
        let (read_end, mut write_end) = std::io::pipe().expect("failed to create pipe");
        writeln!(write_end, "ping").expect("failed to write to pipe");
        drop(write_end);

        let remaps = vec![FdRemap {
            source: read_end.into(),
            target_fd: 101,
        }];

        let mut command = Command::new("/bin/sh");
        command
            .arg("-c")
            .arg("IFS= read -r line <&101; [ \"$line\" = \"ping\" ]");
        super::configure_fd_remaps(&mut command, remaps)
            .expect("expected configure_fd_remaps to succeed");

        let status = command.status().expect("failed to spawn child");
        assert!(status.success());
    }
}
