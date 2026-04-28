#![doc = include_str!("../../../README.md")]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};

use anyhow::{Context, Result};
use lockin_process::CommandFdExt;

mod paths;

#[cfg(feature = "tokio")]
pub mod tokio;

#[cfg(target_os = "macos")]
mod darwin;
#[cfg(target_os = "linux")]
mod linux;

/// Environment variables that can alter the dynamic linker's behavior
/// in a sandboxed child (preload arbitrary `.so`/`.dylib`s, redirect
/// library lookup, etc.). Every spawn from a [`SandboxedCommand`]
/// strips these vars — explicit `env()`/`envs()` calls drop them at
/// set time, and any value inherited from the parent environment is
/// removed at spawn.
pub(crate) const DYNAMIC_LINKER_ENV_BLOCKLIST: &[&str] = &[
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "LD_AUDIT",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    "DYLD_FRAMEWORK_PATH",
];

/// Panics if `path` contains any ASCII control byte (0x00..=0x1F or
/// 0x7F). Such bytes are valid in unix paths but would be embedded
/// literally into Seatbelt `(literal "...")` strings or syd path
/// rules, where a `\n` could split a single rule into two and turn a
/// `(deny ...)` into a no-op or a stray `(allow ...)`. We refuse them
/// at the API boundary so every backend rule emitter sees clean
/// input.
fn assert_no_control_chars(method: &str, path: &Path) {
    use std::os::unix::ffi::OsStrExt;
    let bytes = path.as_os_str().as_bytes();
    if bytes.iter().any(|b| *b < 0x20 || *b == 0x7F) {
        panic!(
            "{method} must not contain control characters, got: {:?}",
            path
        );
    }
}

pub(crate) fn is_dynamic_linker_blocked(key: &OsStr) -> bool {
    let bytes = key.as_encoded_bytes();
    DYNAMIC_LINKER_ENV_BLOCKLIST
        .iter()
        .any(|blocked| bytes == blocked.as_bytes())
}

/// Network enforcement strategy for the sandboxed child.
///
/// `Deny` is the default. It denies IP networking (TCP/UDP, v4 and
/// v6), inbound bind/listen, and AF_UNIX outbound to arbitrary
/// paths. On macOS a small set of Apple system services required
/// for normal program startup remains reachable; programs can use
/// them but cannot register new Mach names, look up arbitrary XPC
/// services, write to `/cores`, or connect to the syslog Unix
/// socket. `AllowAll` removes all network restrictions; for
/// workloads that manage their own networking and need full
/// passthrough. Rare — most users want `Deny` or `Proxy`. `Proxy`
/// allows outbound only to a loopback port, where lockin's caller
/// has stood up an `outpost-proxy` instance enforcing a host
/// allowlist via HTTP CONNECT — apps in the sandbox see their
/// traffic filtered per-hostname if they honor `HTTP_PROXY`, and
/// fail closed at the sandbox if they don't.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NetworkMode {
    #[default]
    Deny,
    AllowAll,
    Proxy {
        loopback_port: u16,
    },
}

/// Internal representation of sandbox policy. Not part of the public
/// API; use [`SandboxBuilder`] to configure a sandbox.
#[derive(Debug, Clone, Default)]
pub(crate) struct SandboxSpec {
    pub(crate) network: NetworkMode,
    pub(crate) allow_kvm: bool,
    pub(crate) allow_interactive_tty: bool,
    pub(crate) allow_non_pie_exec: bool,
    pub(crate) syd_path: Option<PathBuf>,
    pub(crate) read_paths: Vec<PathBuf>,
    pub(crate) read_dirs: Vec<PathBuf>,
    pub(crate) write_paths: Vec<PathBuf>,
    pub(crate) write_dirs: Vec<PathBuf>,
    pub(crate) exec_paths: Vec<PathBuf>,
    pub(crate) exec_dirs: Vec<PathBuf>,
    pub(crate) rlimits: Vec<(i32, u64)>,
    pub(crate) raw_seatbelt_rules: Vec<String>,
}

/// A prepared sandbox environment: holds the private tmp directory and
/// any per-platform state needed to build sandboxed commands.
///
/// Construct one via [`Sandbox::builder`] and
/// [`SandboxBuilder::command`].
///
/// ```
/// use std::path::Path;
/// use lockin::Sandbox;
///
/// let status = Sandbox::builder()
///     .command(Path::new("/usr/bin/env"))
///     .unwrap()
///     .status()
///     .unwrap();
/// assert!(status.success());
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
        let syd = resolve_syd_path(&spec)?;

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
        anyhow::bail!("lockin: sandboxing is not implemented for this platform")
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
    pub(crate) fn build_command(&self, program: &Path) -> Command {
        linux::build_sandbox_command(&self.spec, self.private_tmp.path(), &self.syd, program)
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn build_command(&self, program: &Path) -> Command {
        darwin::build_sandbox_command(&self.spec, self.private_tmp.path(), program)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    pub(crate) fn build_command(&self, _program: &Path) -> Command {
        unreachable!("Sandbox::new fails on unsupported platforms")
    }
}

#[cfg(target_os = "linux")]
fn resolve_syd_path(spec: &SandboxSpec) -> Result<PathBuf> {
    if let Some(path) = &spec.syd_path {
        return Ok(path.clone());
    }

    if let Some(val) = std::env::var_os("LOCKIN_SYD_PATH") {
        let path = PathBuf::from(val);
        anyhow::ensure!(
            path.is_absolute(),
            "LOCKIN_SYD_PATH must be absolute, got: {}",
            path.display()
        );
        return Ok(path);
    }

    if let Some(path) = find_in_path("syd") {
        return Ok(path);
    }

    anyhow::bail!(
        "Linux sandbox requires syd but could not find it. \
         Set LOCKIN_SYD_PATH, add syd to PATH, or call .syd_path() explicitly."
    )
}

#[cfg(target_os = "linux")]
fn find_in_path(binary: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;

    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        if !dir.is_absolute() {
            continue;
        }
        let candidate = dir.join(binary);
        if let Ok(meta) = candidate.metadata() {
            if meta.is_file() && meta.permissions().mode() & 0o111 != 0 {
                return Some(candidate);
            }
        }
    }
    None
}

fn create_private_tmp() -> Result<tempfile::TempDir> {
    let base = std::env::temp_dir().join("lockin");
    std::fs::create_dir_all(&base)
        .with_context(|| format!("failed to create sandbox temp base {}", base.display()))?;

    tempfile::Builder::new()
        .prefix("sbx-")
        .tempdir_in(&base)
        .with_context(|| format!("failed to create private temp dir in {}", base.display()))
}

/// Fluent builder for a sandboxed child command.
///
/// Configure the sandbox policy and any fds to inherit into the
/// child, then call [`SandboxBuilder::command`] to get a
/// [`SandboxedCommand`] ready to spawn.
///
/// # Example
///
/// ```
/// use std::os::fd::{AsRawFd, OwnedFd};
/// use std::path::Path;
/// use lockin::Sandbox;
///
/// fn spawn_with_ready_pipe(ready_writer: OwnedFd) -> anyhow::Result<()> {
///     let ready_raw = ready_writer.as_raw_fd();
///
///     let mut builder = Sandbox::builder().network_allow_all();
///     builder.inherit_fd(ready_writer);
///
///     let mut cmd = builder.command(Path::new("/usr/bin/capsa-netd"))?;
///     cmd.arg(format!("--ready-fd={ready_raw}"));
///     cmd.spawn()?;
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

    /// Sets the network enforcement mode directly. Prefer the
    /// dedicated helpers ([`network_deny`](Self::network_deny),
    /// [`network_allow_all`](Self::network_allow_all),
    /// [`network_proxy`](Self::network_proxy)) for readable call
    /// sites.
    pub fn network(mut self, mode: NetworkMode) -> Self {
        self.spec.network = mode;
        self
    }

    /// Denies all network access from the sandboxed child. This is
    /// the default.
    pub fn network_deny(self) -> Self {
        self.network(NetworkMode::Deny)
    }

    /// Allows unrestricted inbound and outbound networking. For
    /// workloads that manage their own networking and need full
    /// passthrough; rare. Most users want `Deny` or `Proxy`.
    pub fn network_allow_all(self) -> Self {
        self.network(NetworkMode::AllowAll)
    }

    /// Allows outbound network only to `127.0.0.1:loopback_port`, where
    /// the caller is expected to have stood up an HTTP CONNECT proxy
    /// (see the `outpost-proxy` crate). All other outbound traffic is
    /// denied at the OS sandbox layer, so apps that ignore
    /// `HTTP_PROXY` fail closed rather than silently leaking.
    pub fn network_proxy(self, loopback_port: u16) -> Self {
        self.network(NetworkMode::Proxy { loopback_port })
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

    /// Permits exec of non-PIE (not Position Independent Executable)
    /// binaries. syd denies these by default as ROP-style exploit
    /// hardening, which breaks toolchains whose compilers are built
    /// without `-fPIE` (notably `gcc`/`rustc` on Nix). Linux only;
    /// ignored on macOS.
    pub fn allow_non_pie_exec(mut self, allow: bool) -> Self {
        self.spec.allow_non_pie_exec = allow;
        self
    }

    /// Sets the absolute path to the `syd` sandbox enforcer binary
    /// (Linux only; ignored on other platforms).
    ///
    /// If not set, the library checks `LOCKIN_SYD_PATH` then `PATH`.
    ///
    /// Panics if `path` is not absolute.
    pub fn syd_path(mut self, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        assert!(
            path.is_absolute(),
            "syd_path must be absolute, got: {}",
            path.display()
        );
        assert_no_control_chars("syd_path", &path);
        self.spec.syd_path = Some(path);
        self
    }

    /// Adds a single file path that the child should be allowed to
    /// read. Use [`read_dir`](Self::read_dir) for
    /// directories that need recursive access.
    ///
    /// Panics if `path` is not absolute.
    pub fn read_path(mut self, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        assert!(
            path.is_absolute(),
            "read_path must be absolute, got: {}",
            path.display()
        );
        assert_no_control_chars("read_path", &path);
        self.spec.read_paths.push(path);
        self
    }

    /// Adds a directory whose contents the child should be allowed to
    /// read recursively.
    ///
    /// Panics if `path` is not absolute.
    pub fn read_dir(mut self, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        assert!(
            path.is_absolute(),
            "read_dir must be absolute, got: {}",
            path.display()
        );
        assert_no_control_chars("read_dir", &path);
        self.spec.read_dirs.push(path);
        self
    }

    /// Adds a single file path that the child should be allowed to
    /// read and write. Use [`write_dir`](Self::write_dir)
    /// for directories that need recursive access.
    ///
    /// Panics if `path` is not absolute.
    pub fn write_path(mut self, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        assert!(
            path.is_absolute(),
            "write_path must be absolute, got: {}",
            path.display()
        );
        assert_no_control_chars("write_path", &path);
        self.spec.write_paths.push(path);
        self
    }

    /// Adds a directory whose contents the child should be allowed to
    /// read and write recursively.
    ///
    /// Panics if `path` is not absolute.
    pub fn write_dir(mut self, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        assert!(
            path.is_absolute(),
            "write_dir must be absolute, got: {}",
            path.display()
        );
        assert_no_control_chars("write_dir", &path);
        self.spec.write_dirs.push(path);
        self
    }

    /// Adds a single file path that the child should be allowed to
    /// execute. Implies read access on the same path.
    ///
    /// Panics if `path` is not absolute.
    pub fn exec_path(mut self, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        assert!(
            path.is_absolute(),
            "exec_path must be absolute, got: {}",
            path.display()
        );
        assert_no_control_chars("exec_path", &path);
        self.spec.exec_paths.push(path);
        self
    }

    /// Adds a directory whose contents the child should be allowed to
    /// execute recursively. Implies recursive read access on the same
    /// directory.
    ///
    /// Panics if `path` is not absolute.
    pub fn exec_dir(mut self, path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        assert!(
            path.is_absolute(),
            "exec_dir must be absolute, got: {}",
            path.display()
        );
        assert_no_control_chars("exec_dir", &path);
        self.spec.exec_dirs.push(path);
        self
    }

    /// Appends a raw sandbox-exec (Seatbelt) S-expression rule to the
    /// generated darwin profile. Rules are emitted verbatim after the
    /// structured allows.
    ///
    /// **Trusted-policy escape hatch.** Any `(allow ...)` rule passed
    /// here grants the child whatever authority the rule names — it
    /// can broaden the sandbox arbitrarily, including beyond what the
    /// structured builder methods would permit. Raw rules can also
    /// invoke named bundles defined by the macOS system profile but
    /// not enabled by default: a single `(system-network)` token
    /// unlocks routing-socket egress, mDNS, and the network-extension
    /// service surface; `(system-graphics)` unlocks the IOKit GPU
    /// surface. The caller is responsible for the safety of every
    /// rule passed in; treat this as policy code, not configuration.
    ///
    /// Intended for darwin-specific operations not expressible
    /// through the structured API (e.g. `iokit-open`, `mach-lookup`,
    /// `sysctl-read`). Malformed rules make `sandbox-exec` reject the
    /// profile at spawn time.
    ///
    /// Ignored on non-darwin platforms.
    pub fn raw_seatbelt_rule(mut self, rule: impl Into<String>) -> Self {
        self.spec.raw_seatbelt_rules.push(rule.into());
        self
    }

    /// Sets `RLIMIT_NOFILE` (max open file descriptors) for the child.
    #[allow(clippy::unnecessary_cast)] // i32 on macOS, u32 on Linux
    pub fn max_open_files(mut self, n: u64) -> Self {
        self.spec.rlimits.push((libc::RLIMIT_NOFILE as i32, n));
        self
    }

    /// Sets `RLIMIT_AS` (max virtual address space in bytes) for the child.
    ///
    /// On macOS the limit is applied to the calling process before
    /// `execve`-ing `sandbox-exec`, which then `exec`s the user program.
    /// Because rlimits are inherited across `exec`, the cap applies to
    /// `sandbox-exec`'s own address space first; values too tight to fit
    /// `sandbox-exec` itself will cause the spawn to fail before the user
    /// program ever runs. Size the limit for the larger of the two
    /// processes, not just the user program.
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

    /// Consumes the builder and produces a [`SandboxedCommand`] ready
    /// to spawn `program`.
    ///
    /// The returned `SandboxedCommand` owns both the configured
    /// [`std::process::Command`] and the [`Sandbox`] (private tmp
    /// directory), so callers no longer need to hold a separate
    /// `_sandbox` binding to keep the tmp directory alive.
    ///
    /// Panics if `program` is not absolute.
    pub fn command(self, program: &Path) -> Result<SandboxedCommand> {
        assert!(
            program.is_absolute(),
            "command path must be absolute, got: {}",
            program.display()
        );
        assert_no_control_chars("command", program);
        let (command, sandbox) = self.build(program)?;
        Ok(SandboxedCommand { command, sandbox })
    }

    fn build(self, program: &Path) -> Result<(Command, Sandbox)> {
        #[cfg(not(target_os = "linux"))]
        let rlimits = self.spec.rlimits.clone();

        let sandbox = Sandbox::new(self.spec)?;
        let mut command = sandbox.build_command(program);

        command.seal_fds();
        for (fd, _raw) in self.fds {
            command.keep_fd(fd);
        }

        #[cfg(not(target_os = "linux"))]
        configure_rlimits(&mut command, rlimits)?;

        for key in DYNAMIC_LINKER_ENV_BLOCKLIST {
            command.env_remove(key);
        }

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
    /// Equivalent to [`SandboxBuilder::new`]. Configure policy and
    /// inherited fds on the returned builder, then call
    /// [`SandboxBuilder::command`] to get a [`SandboxedCommand`]
    /// ready to spawn.
    pub fn builder() -> SandboxBuilder {
        SandboxBuilder::new()
    }
}

/// A sandboxed command ready to spawn.
///
/// Wraps a [`std::process::Command`] together with the [`Sandbox`]
/// that owns the private tmp directory. The sandbox stays alive as
/// long as this value does, so callers don't need a separate
/// `_sandbox` binding.
///
/// Methods mirror [`std::process::Command`]'s API. Mutation is only
/// possible through `SandboxedCommand`'s own methods, so the
/// dynamic-linker env strip cannot be bypassed.
pub struct SandboxedCommand {
    command: Command,
    sandbox: Sandbox,
}

impl SandboxedCommand {
    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.command.arg(arg);
        self
    }

    pub fn args(&mut self, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> &mut Self {
        self.command.args(args);
        self
    }

    /// Sets a child env var. Keys in the dynamic-linker blocklist
    /// (e.g. `LD_PRELOAD`, `DYLD_INSERT_LIBRARIES`) are silently
    /// dropped — the sandbox guarantees they do not reach the child.
    pub fn env(&mut self, key: impl AsRef<OsStr>, val: impl AsRef<OsStr>) -> &mut Self {
        let key = key.as_ref();
        if !is_dynamic_linker_blocked(key) {
            self.command.env(key, val);
        }
        self
    }

    /// Sets a batch of child env vars. Entries whose key is in the
    /// dynamic-linker blocklist are silently dropped.
    pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        for (k, v) in vars {
            self.env(k, v);
        }
        self
    }

    pub fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Self {
        self.command.env_remove(key);
        self
    }

    /// Clears the inherited parent environment. The dynamic-linker
    /// blocklist is re-applied right before spawn as defense in depth.
    pub fn env_clear(&mut self) -> &mut Self {
        self.command.env_clear();
        self
    }

    pub fn current_dir(&mut self, dir: impl AsRef<Path>) -> &mut Self {
        self.command.current_dir(dir);
        self
    }

    pub fn stdin(&mut self, cfg: impl Into<Stdio>) -> &mut Self {
        self.command.stdin(cfg);
        self
    }

    pub fn stdout(&mut self, cfg: impl Into<Stdio>) -> &mut Self {
        self.command.stdout(cfg);
        self
    }

    pub fn stderr(&mut self, cfg: impl Into<Stdio>) -> &mut Self {
        self.command.stderr(cfg);
        self
    }

    pub fn status(&mut self) -> std::io::Result<ExitStatus> {
        self.strip_dynamic_linker_env();
        self.command.status()
    }

    pub fn output(&mut self) -> std::io::Result<Output> {
        self.strip_dynamic_linker_env();
        self.command.output()
    }

    fn strip_dynamic_linker_env(&mut self) {
        for key in DYNAMIC_LINKER_ENV_BLOCKLIST {
            self.command.env_remove(key);
        }
    }

    /// Spawns the sandboxed child, transferring sandbox ownership to
    /// the returned [`SandboxedChild`].
    pub fn spawn(mut self) -> std::io::Result<SandboxedChild> {
        self.strip_dynamic_linker_env();
        let child = self.command.spawn()?;
        Ok(SandboxedChild {
            child,
            sandbox: self.sandbox,
        })
    }

    /// Read-only access to the underlying [`std::process::Command`]
    /// for inspection (e.g. `get_envs`, `get_args`). Mutation is only
    /// possible through `SandboxedCommand`'s own methods, which is what
    /// preserves the dynamic-linker env strip.
    pub fn as_command(&self) -> &Command {
        &self.command
    }

    /// Registers a closure to be run in the child after `fork` and
    /// before `exec`.
    ///
    /// # Safety
    ///
    /// Same safety contract as
    /// [`std::os::unix::process::CommandExt::pre_exec`]: the closure
    /// must only call async-signal-safe operations, must not allocate,
    /// and must not touch shared mutable state.
    #[cfg(unix)]
    pub unsafe fn pre_exec<F>(&mut self, f: F) -> &mut Self
    where
        F: FnMut() -> std::io::Result<()> + Send + Sync + 'static,
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            self.command.pre_exec(f);
        }
        self
    }
}

/// A running sandboxed child process.
///
/// Wraps a [`std::process::Child`] together with the [`Sandbox`]
/// whose private tmp directory must outlive the child. Dropping
/// this value cleans up the tmp directory (but does **not**
/// kill the child — call [`kill`](SandboxedChild::kill) first if
/// needed).
pub struct SandboxedChild {
    child: Child,
    sandbox: Sandbox,
}

impl SandboxedChild {
    pub fn wait(&mut self) -> std::io::Result<ExitStatus> {
        self.child.wait()
    }

    pub fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    pub fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill()
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    pub fn as_child(&self) -> &Child {
        &self.child
    }

    pub fn as_child_mut(&mut self) -> &mut Child {
        &mut self.child
    }

    pub fn into_parts(self) -> (Child, Sandbox) {
        (self.child, self.sandbox)
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
    #[cfg(target_os = "linux")]
    use std::sync::Mutex;

    #[cfg(target_os = "linux")]
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[cfg(target_os = "linux")]
    struct EnvGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
        saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    #[cfg(target_os = "linux")]
    impl EnvGuard {
        fn lock(vars: &[&'static str]) -> Self {
            let _lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            let saved = vars.iter().map(|&k| (k, std::env::var_os(k))).collect();
            Self { _lock, saved }
        }
    }

    #[cfg(target_os = "linux")]
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, val) in &self.saved {
                match val {
                    Some(v) => std::env::set_var(key, v),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

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

        let mut builder = super::SandboxBuilder::new().network_allow_all();
        let a_raw = builder.inherit_fd(a);
        let b_raw = builder.inherit_fd(b);
        assert_eq!(a_raw, a_raw_before);
        assert_eq!(b_raw, b_raw_before);
    }

    #[test]
    fn syd_path_builder_stores_absolute() {
        let builder = super::SandboxBuilder::new().syd_path("/usr/bin/syd");
        assert_eq!(
            builder.spec.syd_path,
            Some(std::path::PathBuf::from("/usr/bin/syd"))
        );
    }

    #[test]
    #[should_panic(expected = "syd_path must be absolute")]
    fn syd_path_builder_rejects_relative() {
        super::SandboxBuilder::new().syd_path("bin/syd");
    }

    #[test]
    #[should_panic(expected = "read_path must be absolute")]
    fn read_path_builder_rejects_relative() {
        super::SandboxBuilder::new().read_path("etc/passwd");
    }

    #[test]
    #[should_panic(expected = "read_dir must be absolute")]
    fn read_dir_builder_rejects_relative() {
        super::SandboxBuilder::new().read_dir("etc");
    }

    #[test]
    #[should_panic(expected = "write_path must be absolute")]
    fn write_path_builder_rejects_relative() {
        super::SandboxBuilder::new().write_path("tmp/out");
    }

    #[test]
    #[should_panic(expected = "write_dir must be absolute")]
    fn write_dir_builder_rejects_relative() {
        super::SandboxBuilder::new().write_dir("tmp");
    }

    #[test]
    #[should_panic(expected = "exec_path must be absolute")]
    fn exec_path_builder_rejects_relative() {
        super::SandboxBuilder::new().exec_path("bin/true");
    }

    #[test]
    #[should_panic(expected = "exec_dir must be absolute")]
    fn exec_dir_builder_rejects_relative() {
        super::SandboxBuilder::new().exec_dir("bin");
    }

    #[test]
    #[should_panic(expected = "command path must be absolute")]
    fn command_builder_rejects_relative() {
        let _ = super::SandboxBuilder::new().command(std::path::Path::new("bin/echo"));
    }

    #[test]
    #[should_panic(expected = "control characters")]
    fn read_path_builder_rejects_newline() {
        super::SandboxBuilder::new().read_path("/tmp/foo\nbar");
    }

    #[test]
    #[should_panic(expected = "control characters")]
    fn read_dir_builder_rejects_carriage_return() {
        super::SandboxBuilder::new().read_dir("/tmp/foo\rbar");
    }

    #[test]
    #[should_panic(expected = "control characters")]
    fn write_path_builder_rejects_nul() {
        super::SandboxBuilder::new().write_path("/tmp/foo\0bar");
    }

    #[test]
    #[should_panic(expected = "control characters")]
    fn write_dir_builder_rejects_tab() {
        super::SandboxBuilder::new().write_dir("/tmp/foo\tbar");
    }

    #[test]
    #[should_panic(expected = "control characters")]
    fn exec_path_builder_rejects_del() {
        super::SandboxBuilder::new().exec_path("/tmp/foo\x7fbar");
    }

    #[test]
    #[should_panic(expected = "control characters")]
    fn exec_dir_builder_rejects_newline() {
        super::SandboxBuilder::new().exec_dir("/tmp/foo\nbar");
    }

    #[test]
    #[should_panic(expected = "control characters")]
    fn syd_path_builder_rejects_newline() {
        super::SandboxBuilder::new().syd_path("/usr/bin/syd\nrogue");
    }

    #[test]
    #[should_panic(expected = "control characters")]
    fn command_builder_rejects_newline() {
        let _ = super::SandboxBuilder::new().command(std::path::Path::new("/bin/echo\n(allow)"));
    }

    #[test]
    fn read_path_accepts_paths_with_spaces_parens_and_unicode() {
        let builder = super::SandboxBuilder::new()
            .read_path("/tmp/with space")
            .read_path("/tmp/with (parens)")
            .read_path("/tmp/café");
        assert_eq!(builder.spec.read_paths.len(), 3);
    }

    #[cfg(target_os = "linux")]
    mod resolve_syd {
        use super::super::*;
        use super::EnvGuard;

        #[test]
        fn explicit_path_takes_priority() {
            let _g = EnvGuard::lock(&["LOCKIN_SYD_PATH"]);
            std::env::set_var("LOCKIN_SYD_PATH", "/should/not/be/used");
            let spec = SandboxSpec {
                syd_path: Some(PathBuf::from("/explicit/syd")),
                ..Default::default()
            };
            assert_eq!(
                resolve_syd_path(&spec).unwrap(),
                PathBuf::from("/explicit/syd")
            );
        }

        #[test]
        fn env_var_used_when_no_explicit_path() {
            let _g = EnvGuard::lock(&["LOCKIN_SYD_PATH"]);
            std::env::set_var("LOCKIN_SYD_PATH", "/from/env/syd");
            assert_eq!(
                resolve_syd_path(&SandboxSpec::default()).unwrap(),
                PathBuf::from("/from/env/syd")
            );
        }

        #[test]
        fn env_var_rejects_relative_path() {
            let _g = EnvGuard::lock(&["LOCKIN_SYD_PATH"]);
            std::env::set_var("LOCKIN_SYD_PATH", "relative/syd");
            let err = resolve_syd_path(&SandboxSpec::default()).unwrap_err();
            assert!(err.to_string().contains("must be absolute"));
        }

        #[test]
        fn falls_back_to_path_lookup() {
            let _g = EnvGuard::lock(&["LOCKIN_SYD_PATH", "PATH"]);
            std::env::remove_var("LOCKIN_SYD_PATH");

            let dir = tempfile::tempdir().unwrap();
            let syd = dir.path().join("syd");
            std::fs::write(&syd, "").unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&syd, std::fs::Permissions::from_mode(0o755)).unwrap();
            }

            std::env::set_var("PATH", dir.path());
            assert_eq!(resolve_syd_path(&SandboxSpec::default()).unwrap(), syd);
        }

        #[test]
        fn error_when_syd_not_found() {
            let _g = EnvGuard::lock(&["LOCKIN_SYD_PATH", "PATH"]);
            std::env::remove_var("LOCKIN_SYD_PATH");
            std::env::set_var("PATH", "/nonexistent");
            assert!(resolve_syd_path(&SandboxSpec::default()).is_err());
        }
    }
}
