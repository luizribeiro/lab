use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{ensure, Context, Result};

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
#[derive(Debug, Clone)]
pub(crate) struct SandboxSpec {
    pub(crate) allow_network: bool,
    pub(crate) allow_kvm: bool,
    pub(crate) allow_interactive_tty: bool,
    pub(crate) close_non_inherited_fds: bool,
    pub(crate) read_only_paths: Vec<PathBuf>,
    pub(crate) read_write_paths: Vec<PathBuf>,
    pub(crate) ioctl_paths: Vec<PathBuf>,
    pub(crate) rlimits: Vec<(i32, u64)>,
}

impl Default for SandboxSpec {
    fn default() -> Self {
        Self {
            allow_network: false,
            allow_kvm: false,
            allow_interactive_tty: false,
            close_non_inherited_fds: true,
            read_only_paths: Vec::new(),
            read_write_paths: Vec::new(),
            ioctl_paths: Vec::new(),
            rlimits: Vec::new(),
        }
    }
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
///     // Read the raw fd number before moving ownership so we can
///     // tell the child which fd to use.
///     let ready_raw = ready_writer.as_raw_fd();
///
///     let mut builder = Sandbox::builder().allow_network(true);
///     builder.inherit_fd(ready_writer)?;
///
///     let (mut cmd, _sandbox) = builder.build(Path::new("/usr/bin/capsa-netd"))?;
///     cmd.arg(format!("--ready-fd={ready_raw}"));
///     let _child = cmd.spawn()?;
///     Ok(())
/// }
/// ```
pub struct SandboxBuilder {
    spec: SandboxSpec,
    inherited_fds: Vec<std::os::fd::OwnedFd>,
    inherited_raws: std::collections::HashSet<std::os::fd::RawFd>,
}

impl SandboxBuilder {
    /// Creates a new builder with an empty sandbox policy and no
    /// inherited fds.
    pub fn new() -> Self {
        Self {
            spec: SandboxSpec::default(),
            inherited_fds: Vec::new(),
            inherited_raws: std::collections::HashSet::new(),
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

    /// Controls whether the pre_exec hook closes all file descriptors
    /// `>= 3` that were **not** registered via [`SandboxBuilder::inherit_fd`].
    ///
    /// Enabled by default. This prevents leaked privileged fds from
    /// becoming sandbox escape vectors.
    pub fn close_non_inherited_fds(mut self, close: bool) -> Self {
        self.spec.close_non_inherited_fds = close;
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
    /// Validation:
    /// - `fd.as_raw_fd() >= 3` (fds 0, 1, 2 are reserved for stdio)
    /// - no duplicate raw fd numbers within one builder (caught as a
    ///   safety net for unsafe `OwnedFd::from_raw_fd` aliasing bugs)
    ///
    /// Takes `&mut self` (not `self`) because it returns the raw fd
    /// number; callers typically use this inside a loop rather than
    /// as part of a fluent chain.
    pub fn inherit_fd(&mut self, fd: std::os::fd::OwnedFd) -> Result<std::os::fd::RawFd> {
        use std::os::fd::AsRawFd;

        let raw = fd.as_raw_fd();
        ensure!(
            raw >= 3,
            "inherit_fd: refusing to inherit fd {raw}; fds 0, 1, and 2 are reserved for stdio"
        );
        ensure!(
            self.inherited_raws.insert(raw),
            "inherit_fd: fd {raw} already inherited; each fd can only be inherited once"
        );
        self.inherited_fds.push(fd);
        Ok(raw)
    }

    /// Consumes the builder and returns the inherited fds and the
    /// `close_non_inherited_fds` flag for the sandbox-bypass path.
    ///
    /// Intended for callers that skip the sandbox wrapper (e.g. the
    /// `CAPSA_DISABLE_SANDBOX` escape hatch) but still need fd
    /// inheritance for socketpair communication.
    pub fn into_inherited_fds_config(self) -> (Vec<std::os::fd::OwnedFd>, bool) {
        (self.inherited_fds, self.spec.close_non_inherited_fds)
    }

    /// Consumes the builder and produces a `(Command, Sandbox)` pair
    /// ready to spawn `program`.
    ///
    /// The `Command` is pre-wired with the sandbox wrapping (syd on
    /// Linux, sandbox-exec on macOS) and with a `pre_exec` hook that
    /// clears `FD_CLOEXEC` on each inherited fd so the fds survive
    /// `exec` at their current kernel-assigned numbers in the child.
    /// The returned `Sandbox` owns a private tmp directory that must
    /// outlive the spawned `Child`; the caller should store it next to
    /// the `Child` handle (e.g. via `DaemonProcess` in capsa-core).
    pub fn build(self, program: &Path) -> Result<(Command, Sandbox)> {
        let close_non_inherited = self.spec.close_non_inherited_fds;

        // On non-Linux platforms, rlimits are enforced via a pre_exec
        // setrlimit hook since there is no sandbox wrapper that
        // handles them natively.
        #[cfg(not(target_os = "linux"))]
        let rlimits = self.spec.rlimits.clone();

        let sandbox = Sandbox::new(self.spec)?;
        let mut command = sandbox.command(program);
        configure_inherited_fds(&mut command, self.inherited_fds, close_non_inherited)?;

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

/// Reads `/dev/fd` (macOS) or `/proc/self/fd` (Linux) to discover
/// currently-open fds, then returns those `>= 3` that are NOT in
/// `inherited`. Called in the parent before fork so directory I/O
/// is safe.
fn enumerate_non_inherited_fds(
    inherited: &std::collections::HashSet<i32>,
) -> Vec<std::os::fd::RawFd> {
    let fd_dir = if cfg!(target_os = "linux") {
        "/proc/self/fd"
    } else {
        "/dev/fd"
    };

    let entries = match std::fs::read_dir(fd_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    entries
        .filter_map(|entry| {
            let name = entry.ok()?.file_name();
            let fd: i32 = name.to_str()?.parse().ok()?;
            if fd >= 3 && !inherited.contains(&fd) {
                Some(fd)
            } else {
                None
            }
        })
        .collect()
}

/// Installs a `pre_exec` hook on `command` that makes the given
/// owned fds survive `exec` in the child at their current raw fd
/// numbers, by clearing `FD_CLOEXEC` on each one.
///
/// When `close_non_inherited` is true, the hook also sets `FD_CLOEXEC`
/// on every fd `>= 3` that is **not** in the inherited set, so they
/// are closed at exec time. This prevents leaked privileged fds from
/// becoming sandbox escape vectors without disturbing Rust std's
/// internal error-reporting pipe.
///
/// This is the lower-level primitive used by [`SandboxBuilder::build`].
/// It is exposed separately so callers that need fd inheritance
/// without a sandbox wrapper (for example, the `CAPSA_DISABLE_SANDBOX`
/// bypass path in capsa-core) can apply the same pre_exec hook to a
/// plain `std::process::Command`.
///
/// The `OwnedFd`s are moved into the pre_exec closure, which keeps
/// them alive in the parent until spawn and closes them automatically
/// (via `OwnedFd::drop`) when the `Command` is dropped. In the child,
/// after `exec`, the fds remain open at their current numbers because
/// their close-on-exec flag has been cleared.
///
/// Validation (performed up-front, before the `pre_exec` hook is
/// installed):
/// - each fd's raw number must be `>= 3`
/// - no duplicate raw fd numbers
pub fn configure_inherited_fds(
    command: &mut Command,
    fds: Vec<std::os::fd::OwnedFd>,
    close_non_inherited: bool,
) -> Result<()> {
    use std::collections::HashSet;
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;

    if fds.is_empty() && !close_non_inherited {
        return Ok(());
    }

    let mut seen = HashSet::new();
    for (index, fd) in fds.iter().enumerate() {
        let raw = fd.as_raw_fd();
        ensure!(
            raw >= 3,
            "configure_inherited_fds[{index}]: refusing to inherit fd {raw}; fds 0, 1, and 2 are reserved for stdio"
        );
        ensure!(
            seen.insert(raw),
            "configure_inherited_fds[{index}]: fd {raw} already present; each fd can only be inherited once"
        );
    }

    let inherited_set: HashSet<i32> = fds.iter().map(|fd| fd.as_raw_fd()).collect();

    // Snapshot the set of fds to mark FD_CLOEXEC in the parent
    // (before fork) by reading /dev/fd (macOS) or /proc/self/fd
    // (Linux). This is O(open_fds) rather than O(max_fd), avoiding
    // the multi-second stall from iterating sysconf(_SC_OPEN_MAX)
    // which can be ~1M on macOS.
    let fds_to_cloexec = if close_non_inherited {
        enumerate_non_inherited_fds(&inherited_set)
    } else {
        Vec::new()
    };

    let child_hook = move || -> std::io::Result<()> {
        for fd in &fds {
            let raw = fd.as_raw_fd();

            // SAFETY: fcntl with F_GETFD is a read-only query on an
            // integer fd; async-signal-safe per POSIX.1 and cannot
            // cause UB regardless of the fd value.
            let flags = unsafe { libc::fcntl(raw, libc::F_GETFD) };
            if flags == -1 {
                return Err(std::io::Error::last_os_error());
            }

            // SAFETY: fcntl with F_SETFD sets the close-on-exec flag
            // for the given integer fd; async-signal-safe per POSIX.1.
            // The fd is owned by this closure (via the captured
            // `OwnedFd`) so no other owner is racing this update.
            let rc = unsafe { libc::fcntl(raw, libc::F_SETFD, flags & !libc::FD_CLOEXEC) };
            if rc == -1 {
                return Err(std::io::Error::last_os_error());
            }
        }

        for &fd in &fds_to_cloexec {
            // Set FD_CLOEXEC rather than close() so that Rust std's
            // internal error-reporting pipe (used to propagate exec
            // failures back to the parent) survives pre_exec. The
            // fd will be closed automatically at exec time.
            //
            // SAFETY: fcntl with F_SETFD is async-signal-safe per
            // POSIX.1. EBADF on fds closed between snapshot and
            // fork is harmless.
            unsafe {
                libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
            }
        }

        Ok(())
    };

    // SAFETY: pre_exec runs `child_hook` in the child after fork and
    // before exec, where only async-signal-safe work is permitted.
    // The closure's operations are all async-signal-safe:
    //   - iterating pre-allocated `Vec`s by reference (no allocation),
    //   - `OwnedFd::as_raw_fd` (reads an inline integer),
    //   - `libc::fcntl` with `F_GETFD` / `F_SETFD` (POSIX
    //     async-signal-safe list),
    //   - `std::io::Error::last_os_error`, which in the current std
    //     implementation just reads `errno` and stores it in an
    //     inline `Os` variant (no allocation, no locks). This is an
    //     empirical claim about current std; the API does not
    //     formally promise async-signal safety.
    // No Rust destructors run on the happy path: on success the child
    // falls through to `exec`, discarding the Rust stack; on failure
    // the error is returned to std's spawn machinery which reports it
    // and calls `_exit`.
    unsafe {
        command.pre_exec(child_hook);
    }

    Ok(())
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
    use std::io::Write;
    use std::os::fd::{AsRawFd, OwnedFd};
    use std::process::Command;

    fn owned_tempfile() -> OwnedFd {
        tempfile::tempfile()
            .expect("failed to open temp file")
            .into()
    }

    // Note: we don't unit-test the stdio-slot rejection (raw < 3) or
    // the duplicate-raw-fd rejection paths of `inherit_fd` /
    // `configure_inherited_fds` directly. Both defensive checks
    // require constructing an `OwnedFd` at a controlled raw fd number
    // via `from_raw_fd`, and when the validator returns Err the
    // `OwnedFd` is dropped — which calls `close` on that raw fd.
    // For stdio slots this corrupts the test harness's own stdio; for
    // duplicates it double-closes a fd owned by another handle.
    // The checks themselves are a one-line `ensure!` each, so we
    // rely on the happy-path tests below (which exercise the common
    // accept-and-inherit flow) plus code review.

    #[test]
    fn builder_inherit_fd_returns_raw_and_tracks_uniqueness() {
        let a = owned_tempfile();
        let b = owned_tempfile();
        let a_raw_before = a.as_raw_fd();
        let b_raw_before = b.as_raw_fd();
        assert_ne!(a_raw_before, b_raw_before);

        let mut builder = super::SandboxBuilder::new().allow_network(true);
        let a_raw = builder.inherit_fd(a).expect("first inherit");
        let b_raw = builder.inherit_fd(b).expect("second inherit");
        assert_eq!(a_raw, a_raw_before);
        assert_eq!(b_raw, b_raw_before);
    }

    #[test]
    fn inherited_fd_is_accessible_in_child_at_same_number() {
        let (read_end, mut write_end) = std::io::pipe().expect("failed to create pipe");
        writeln!(write_end, "hello").expect("failed to write to pipe");
        drop(write_end);

        let read_owned: OwnedFd = read_end.into();
        let read_raw = read_owned.as_raw_fd();

        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg(format!(
            "IFS= read -r line <&{read_raw}; [ \"$line\" = \"hello\" ]"
        ));
        super::configure_inherited_fds(&mut cmd, vec![read_owned], false)
            .expect("configure_inherited_fds should succeed");

        let status = cmd.status().expect("spawn should succeed");
        assert!(
            status.success(),
            "child should have read 'hello' from inherited fd {read_raw}"
        );
    }
}
