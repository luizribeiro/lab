use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use capsa_process::CommandFdExt;

mod discover;
mod paths;

#[cfg(target_os = "macos")]
mod darwin;
#[cfg(target_os = "linux")]
mod linux;

#[cfg(feature = "tokio")]
pub mod tokio;

/// Internal representation of sandbox policy. Not part of the public
/// API; use [`SandboxBuilder`] to configure a sandbox.
#[derive(Debug, Clone, Default)]
pub(crate) struct SandboxSpec {
    pub(crate) allow_network: bool,
    pub(crate) allow_kvm: bool,
    pub(crate) allow_interactive_tty: bool,
    pub(crate) read_only_paths: Vec<PathBuf>,
    pub(crate) read_write_paths: Vec<PathBuf>,
    pub(crate) ioctl_paths: Vec<PathBuf>,
    pub(crate) rlimits: Vec<(i32, u64)>,
}

/// A prepared sandbox environment: holds the private tmp directory and
/// any per-platform state needed to build sandboxed commands.
///
/// Construct one via [`Sandbox::builder`] and finalize it into a
/// `(Command, Sandbox)` pair with [`SandboxBuilder::build`]. The caller
/// must keep the `Sandbox` alive until the spawned child exits;
/// dropping it earlier removes the private tmp directory out from
/// under the child.
///
/// ```no_run
/// use std::path::Path;
/// use capsa_sandbox::Sandbox;
///
/// let (mut cmd, _sandbox) = Sandbox::builder()
///     .build(Path::new("/bin/true"))
///     .unwrap();
/// let mut child = cmd.spawn().unwrap();
/// child.wait().unwrap();
/// ```
pub struct Sandbox {
    spec: SandboxSpec,
    private_tmp: tempfile::TempDir,
    #[cfg(target_os = "linux")]
    syd: PathBuf,
}

impl Sandbox {
    /// Prepares a sandbox from `spec`. Internal to the crate; use
    /// [`Sandbox::builder`] from outside.
    #[cfg(target_os = "linux")]
    pub(crate) fn new(spec: SandboxSpec) -> Result<Self> {
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
    pub(crate) fn new(spec: SandboxSpec) -> Result<Self> {
        let private_tmp = create_private_tmp()?;

        Ok(Self { spec, private_tmp })
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    pub(crate) fn new(_spec: SandboxSpec) -> Result<Self> {
        anyhow::bail!("capsa-sandbox: sandboxing is not implemented for this platform")
    }

    /// Path of the private tmp directory that the sandbox will expose
    /// to its children as `$TMPDIR`.
    pub fn private_tmp(&self) -> &Path {
        self.private_tmp.path()
    }

    /// Internal helper: builds a [`std::process::Command`] that runs
    /// `program` inside this sandbox. Called by
    /// [`SandboxBuilder::build`]; not part of the public API.
    #[cfg(target_os = "linux")]
    pub(crate) fn command(&self, program: &Path) -> Command {
        linux::build_sandbox_command(&self.spec, self.private_tmp.path(), &self.syd, program)
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn command(&self, program: &Path) -> Command {
        darwin::build_sandbox_command(&self.spec, self.private_tmp.path(), program)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    pub(crate) fn command(&self, _program: &Path) -> Command {
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

/// Fluent builder for a sandboxed child command.
///
/// This is the preferred entry point for the crate. Configure the
/// sandbox policy and any fds to inherit into the child, then call
/// [`SandboxBuilder::build`] to get back a `(Command, Sandbox)` pair.
/// The caller runs `Command::spawn` as usual and keeps the `Sandbox`
/// alive alongside the returned `Child` so the sandbox's private tmp
/// directory outlives the child process.
///
/// # Example
///
/// ```no_run
/// use std::os::fd::{AsRawFd, OwnedFd};
/// use std::path::Path;
/// use capsa_sandbox::Sandbox;
///
/// fn spawn_with_ready_pipe(ready_writer: OwnedFd) -> anyhow::Result<()> {
///     let ready_raw = ready_writer.as_raw_fd();
///
///     let mut builder = Sandbox::builder().allow_network(true);
///     builder.inherit_fd(ready_writer);
///
///     let (mut cmd, _sandbox) = builder.build(Path::new("/usr/bin/capsa-netd"))?;
///     cmd.arg(format!("--ready-fd={ready_raw}"));
///     let _child = cmd.spawn()?;
///     Ok(())
/// }
/// ```
pub struct SandboxBuilder {
    spec: SandboxSpec,
    fds: Vec<(std::os::fd::OwnedFd, std::os::fd::RawFd)>,
}

impl SandboxBuilder {
    /// Creates a new builder with an empty sandbox policy and no
    /// inherited fds.
    pub fn new() -> Self {
        Self {
            spec: SandboxSpec::default(),
            fds: Vec::new(),
        }
    }

    /// Enables or disables outbound/inbound networking from the
    /// sandboxed child.
    pub fn allow_network(mut self, allow: bool) -> Self {
        self.spec.allow_network = allow;
        self
    }

    /// Grants or denies access to `/dev/kvm` and the KVM ioctl set.
    /// Only meaningful on Linux.
    pub fn allow_kvm(mut self, allow: bool) -> Self {
        self.spec.allow_kvm = allow;
        self
    }

    /// Grants or denies access to the caller's controlling terminal
    /// and the terminal ioctl allowlist.
    pub fn allow_interactive_tty(mut self, allow: bool) -> Self {
        self.spec.allow_interactive_tty = allow;
        self
    }

    /// Adds a path that the child should be allowed to read.
    pub fn read_only_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.spec.read_only_paths.push(path.into());
        self
    }

    /// Adds a path that the child should be allowed to read and write.
    pub fn read_write_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.spec.read_write_paths.push(path.into());
        self
    }

    /// Adds a path that the child should be allowed to perform ioctl
    /// operations on.
    pub fn ioctl_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.spec.ioctl_paths.push(path.into());
        self
    }

    /// Sets `RLIMIT_NOFILE` (max open file descriptors) for the child.
    #[allow(clippy::unnecessary_cast)] // i32 on macOS, u32 on Linux
    pub fn max_open_files(mut self, n: u64) -> Self {
        self.spec.rlimits.push((libc::RLIMIT_NOFILE as i32, n));
        self
    }

    /// Sets `RLIMIT_AS` (max virtual address space in bytes) for the child.
    #[allow(clippy::unnecessary_cast)]
    pub fn max_address_space(mut self, bytes: u64) -> Self {
        self.spec.rlimits.push((libc::RLIMIT_AS as i32, bytes));
        self
    }

    /// Sets `RLIMIT_CPU` (max CPU time in seconds) for the child.
    #[allow(clippy::unnecessary_cast)]
    pub fn max_cpu_time(mut self, seconds: u64) -> Self {
        self.spec.rlimits.push((libc::RLIMIT_CPU as i32, seconds));
        self
    }

    /// Sets `RLIMIT_CORE` to 0, preventing the child from dumping core.
    #[allow(clippy::unnecessary_cast)]
    pub fn disable_core_dumps(mut self) -> Self {
        self.spec.rlimits.push((libc::RLIMIT_CORE as i32, 0));
        self
    }

    /// Sets `RLIMIT_NPROC` (max number of processes) for the child.
    #[allow(clippy::unnecessary_cast)]
    pub fn max_processes(mut self, n: u64) -> Self {
        self.spec.rlimits.push((libc::RLIMIT_NPROC as i32, n));
        self
    }

    /// Hands an owned file descriptor to the sandbox to be inherited
    /// into the child at its current raw fd number.
    ///
    /// Returns the raw fd number the child will see the fd at, so the
    /// caller can encode it into argv / env / a launch spec to tell
    /// the child which fd to open.
    ///
    /// Takes `&mut self` (not `self`) because it returns the raw fd
    /// number; callers typically use this inside a loop rather than
    /// as part of a fluent chain.
    pub fn inherit_fd(&mut self, fd: std::os::fd::OwnedFd) -> std::os::fd::RawFd {
        use std::os::fd::AsRawFd;
        let raw = fd.as_raw_fd();
        self.fds.push((fd, raw));
        raw
    }

    /// Consumes the builder and produces a `(Command, Sandbox)` pair
    /// ready to spawn `program`.
    ///
    /// The `Command` is pre-wired with the sandbox wrapping (syd on
    /// Linux, sandbox-exec on macOS) and with `seal_fds` +
    /// `keep_fd` hooks via `capsa-process::CommandFdExt` so that
    /// registered fds survive `exec` while all other fds >= 3 are
    /// closed. The returned `Sandbox` owns a private tmp directory
    /// that must outlive the spawned `Child`.
    pub fn build(self, program: &Path) -> Result<(Command, Sandbox)> {
        #[cfg(not(target_os = "linux"))]
        let rlimits = self.spec.rlimits.clone();

        let sandbox = Sandbox::new(self.spec)?;
        let mut command = sandbox.command(program);

        command.seal_fds();
        for (fd, _raw) in self.fds {
            command.keep_fd(fd);
        }

        #[cfg(not(target_os = "linux"))]
        configure_rlimits(&mut command, rlimits)?;

        Ok((command, sandbox))
    }
}

impl Default for SandboxBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl Sandbox {
    /// Entry point for the fluent sandbox builder API.
    ///
    /// Equivalent to [`SandboxBuilder::new`]. This is the preferred
    /// way to construct a sandbox: configure policy and inherited fds
    /// on the returned builder, then call [`SandboxBuilder::build`] to
    /// realize the sandbox into a `(Command, Sandbox)` pair ready to
    /// spawn.
    pub fn builder() -> SandboxBuilder {
        SandboxBuilder::new()
    }
}

/// Installs a `pre_exec` hook that applies POSIX resource limits via
/// `setrlimit` in the child before `exec`.
///
/// Each entry is a `(resource, limit)` pair where `resource` is a
/// `libc::RLIMIT_*` constant and `limit` is the value to set for both
/// the soft and hard limits.
///
/// Used by [`SandboxBuilder::build`] on non-Linux platforms where
/// there is no sandbox wrapper that handles rlimits natively.
#[cfg(not(target_os = "linux"))]
pub(crate) fn configure_rlimits(command: &mut Command, rlimits: Vec<(i32, u64)>) -> Result<()> {
    use std::os::unix::process::CommandExt;

    if rlimits.is_empty() {
        return Ok(());
    }

    let child_hook = move || -> std::io::Result<()> {
        for &(resource, limit) in &rlimits {
            let rlim = libc::rlimit {
                rlim_cur: limit as libc::rlim_t,
                rlim_max: limit as libc::rlim_t,
            };
            // SAFETY: setrlimit is not on the POSIX async-signal-safe
            // list but is a direct syscall (no heap, no locks) on both
            // Linux and macOS. Stack-local rlimit struct; no allocation.
            let rc = unsafe { libc::setrlimit(resource as _, &rlim) };
            if rc == -1 {
                return Err(std::io::Error::last_os_error());
            }
        }
        Ok(())
    };

    // SAFETY: the closure only calls setrlimit (direct syscall on
    // Linux/macOS, no heap or locks) and iterates a pre-allocated Vec
    // by reference (no allocation).
    unsafe {
        command.pre_exec(child_hook);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::os::fd::AsRawFd;

    fn owned_tempfile() -> std::os::fd::OwnedFd {
        tempfile::tempfile()
            .expect("failed to open temp file")
            .into()
    }

    #[test]
    fn builder_inherit_fd_returns_raw() {
        let a = owned_tempfile();
        let b = owned_tempfile();
        let a_raw_before = a.as_raw_fd();
        let b_raw_before = b.as_raw_fd();
        assert_ne!(a_raw_before, b_raw_before);

        let mut builder = super::SandboxBuilder::new().allow_network(true);
        let a_raw = builder.inherit_fd(a);
        let b_raw = builder.inherit_fd(b);
        assert_eq!(a_raw, a_raw_before);
        assert_eq!(b_raw, b_raw_before);
    }
}
