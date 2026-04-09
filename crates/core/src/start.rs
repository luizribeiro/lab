//! VM startup orchestration.
//!
//! Dispatches to the private `imp` submodule on Linux and macOS. The
//! orchestration (spawn netd, wait for readiness, spawn vmm, wait
//! either) is shared across both; only the per-platform sandbox
//! policy details live behind cfg gates inside `imp`. On platforms
//! other than Linux/macOS, `start()` returns a clear error so
//! downstream crates (`capsa-cli`, tests, etc.) still compile.

use anyhow::Result;

use crate::config::VmConfig;

impl VmConfig {
    /// Start the VM. On Linux and macOS this spawns the daemon
    /// children and blocks until one of them exits. On other
    /// platforms it returns an error immediately because the VM
    /// launch path relies on libkrun, which only supports KVM
    /// (Linux) and HVF (macOS) backends.
    pub fn start(&self) -> Result<()> {
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            imp::start(self)
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = self;
            anyhow::bail!("capsa VM launch is only supported on Linux and macOS")
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod imp {
    use std::io::Read;
    use std::os::fd::{AsRawFd, OwnedFd};
    use std::os::unix::net::UnixDatagram;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use anyhow::{bail, ensure, Context, Result};
    use capsa_net::NetworkPolicy;
    use capsa_sandbox::SandboxBuilder;
    use capsa_spec::{
        encode_launch_spec_args, NetInterfaceSpec, NetLaunchSpec, ResolvedNetworkInterface,
        VmmLaunchSpec,
    };

    use crate::config::{VmConfig, VmNetworkInterfaceConfig};
    use crate::proc::{self, ChildHandle, Exited};

    const READINESS_TIMEOUT: Duration = Duration::from_secs(5);
    const READY_SIGNAL: u8 = b'R';

    /// Paths netd needs to read at runtime (DNS config, cgroup/cpu
    /// info). Carried on the sandbox policy; not part of the
    /// launch-spec contract.
    #[cfg(target_os = "linux")]
    const NETD_RUNTIME_READ_PATHS: &[&str] = &[
        "/etc/resolv.conf",
        "/proc/self/cgroup",
        "/proc/stat",
        "/sys/devices/system/cpu/online",
    ];

    /// macOS has no /proc or /sys; DNS configuration lives in the
    /// SystemConfiguration framework. The exact set of paths
    /// `capsa-net` opens during a darwin run is not yet characterized.
    #[cfg(target_os = "macos")]
    const NETD_RUNTIME_READ_PATHS: &[&str] = &[];

    pub(super) fn start(config: &VmConfig) -> Result<()> {
        config.validate().context("invalid VM configuration")?;

        if config.interfaces.is_empty() {
            return run_vmm_only(config);
        }

        run_with_network(config)
    }

    // ── orchestration ────────────────────────────────────────────

    fn run_vmm_only(config: &VmConfig) -> Result<()> {
        let spec = build_vmm_spec(config, vec![]);
        let vmm = spawn_vmm(&spec, vec![])?;
        let status = vmm
            .wait()
            .context("failed to wait on sandboxed VMM child")?;
        if status.success() {
            Ok(())
        } else {
            bail!("sandboxed VMM process exited with status {status}")
        }
    }

    fn run_with_network(config: &VmConfig) -> Result<()> {
        // Single-interface only; `VmConfig::validate` bails on more.
        // When multi-interface support lands, turn this into a loop
        // over `config.interfaces` that builds `Vec<NetInterfaceSpec>`
        // and `Vec<ResolvedNetworkInterface>` in lockstep.
        debug_assert_eq!(
            config.interfaces.len(),
            1,
            "run_with_network assumes single-interface; config validation should have bailed"
        );
        let iface = &config.interfaces[0];

        let (host_sock, guest_sock) =
            UnixDatagram::pair().context("failed to create interface socketpair")?;
        let host_fd: OwnedFd = host_sock.into();
        let guest_fd: OwnedFd = guest_sock.into();

        let mac = resolve_mac(iface)?;
        let policy = iface.policy.clone();
        let port_forwards = iface.port_forwards.clone();

        let NetdSpawn {
            child: mut netd,
            ready_reader,
        } = spawn_netd(host_fd, mac, policy, port_forwards)?;

        wait_ready(ready_reader, READINESS_TIMEOUT).context("netd readiness check failed")?;

        // If spawn_vmm errors below, `netd` is dropped here and its
        // ChildHandle::Drop tears down the netd child. No explicit
        // cleanup needed -- this is the whole point of RAII handles.
        let vmm_spec = build_vmm_spec(
            config,
            vec![ResolvedNetworkInterface {
                mac,
                guest_fd: 0, // placeholder; `spawn_vmm` rewrites with the kernel-assigned number
            }],
        );
        let mut vmm = spawn_vmm(&vmm_spec, vec![guest_fd])
            .context("failed to spawn sandboxed VMM process")?;

        match proc::wait_either(&mut vmm, &mut netd) {
            Exited::First(Ok(status)) if status.success() => Ok(()),
            Exited::First(Ok(status)) => {
                bail!("sandboxed VMM process exited with status {status}")
            }
            Exited::First(Err(err)) => Err(err).context("failed to reap VMM process"),
            Exited::Second(Ok(status)) => bail!(
                "network daemon exited unexpectedly while VMM was running with status {status}"
            ),
            Exited::Second(Err(err)) => Err(err).context("failed to reap network daemon"),
        }
    }

    // ── netd ─────────────────────────────────────────────────────

    struct NetdSpawn {
        child: ChildHandle,
        ready_reader: OwnedFd,
    }

    fn spawn_netd(
        host_fd: OwnedFd,
        mac: [u8; 6],
        policy: Option<NetworkPolicy>,
        port_forwards: Vec<(u16, u16)>,
    ) -> Result<NetdSpawn> {
        let binary = proc::resolve_binary("CAPSA_NETD_PATH", "capsa-netd")
            .context("failed to resolve net daemon binary")?;
        let (ready_reader, ready_writer) =
            std::io::pipe().context("failed to create netd readiness pipe")?;

        let mut builder = netd_sandbox_builder(&binary);
        let host_fd_num = builder
            .inherit_fd(host_fd)
            .context("failed to inherit netd host fd")?;
        let ready_fd_num = builder
            .inherit_fd(OwnedFd::from(ready_writer))
            .context("failed to inherit netd readiness pipe")?;

        let spec = NetLaunchSpec {
            ready_fd: ready_fd_num,
            interfaces: vec![NetInterfaceSpec {
                host_fd: host_fd_num,
                mac,
                policy,
            }],
            port_forwards,
        };
        spec.validate().context("invalid netd launch spec")?;

        let args = encode_launch_spec_args(&spec)?;
        let child = proc::spawn_sandboxed("netd", &binary, builder, &args, true)
            .context("failed to spawn network daemon")?;

        Ok(NetdSpawn {
            child,
            ready_reader: OwnedFd::from(ready_reader),
        })
    }

    fn netd_sandbox_builder(binary_path: &Path) -> SandboxBuilder {
        let mut builder = capsa_sandbox::Sandbox::builder()
            .allow_network(true)
            .read_only_path(canonical_or_unchanged(binary_path));
        for runtime_read_path in NETD_RUNTIME_READ_PATHS {
            builder = builder.read_only_path(PathBuf::from(*runtime_read_path));
        }
        builder
    }

    /// Block until netd sends the one-byte readiness signal.
    /// Deadline-based so `EINTR` retries do not extend the total wait
    /// beyond `timeout`. Fixes audit finding #2.
    fn wait_ready(reader: OwnedFd, timeout: Duration) -> Result<()> {
        let deadline = Instant::now() + timeout;
        let mut poll_fd = libc::pollfd {
            fd: reader.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                bail!("timed out waiting for net daemon readiness signal");
            }
            let ms = remaining.as_millis().min(i32::MAX as u128) as i32;

            // SAFETY: `poll_fd` points to a valid `pollfd` on the stack.
            let rc = unsafe { libc::poll(&mut poll_fd as *mut libc::pollfd, 1, ms) };
            if rc == 0 {
                bail!("timed out waiting for net daemon readiness signal");
            }
            if rc < 0 {
                let err = std::io::Error::last_os_error();
                if err.raw_os_error() == Some(libc::EINTR) {
                    continue;
                }
                return Err(err).context("poll on net daemon readiness pipe failed");
            }
            break;
        }

        if (poll_fd.revents & libc::POLLIN) == 0 {
            bail!(
                "net daemon readiness pipe became readable without readiness byte (revents={})",
                poll_fd.revents
            );
        }

        let mut ready_file = std::fs::File::from(reader);
        let mut signal = [0u8; 1];
        ready_file
            .read_exact(&mut signal)
            .context("failed reading net daemon readiness byte")?;

        ensure!(
            signal[0] == READY_SIGNAL,
            "invalid net daemon readiness byte: expected {:?}, got {:?}",
            READY_SIGNAL,
            signal[0]
        );

        Ok(())
    }

    // ── vmm ──────────────────────────────────────────────────────

    fn spawn_vmm(spec: &VmmLaunchSpec, guest_fds: Vec<OwnedFd>) -> Result<ChildHandle> {
        ensure!(
            spec.resolved_interfaces.len() == guest_fds.len(),
            "vmm guest_fds count ({}) must match resolved_interfaces ({})",
            guest_fds.len(),
            spec.resolved_interfaces.len()
        );

        let binary = proc::resolve_binary("CAPSA_VMM_PATH", "capsa-vmm")
            .context("failed to resolve VMM binary")?;

        let mut builder = vmm_sandbox_builder(spec, &binary);

        // Inherit each guest fd and record its kernel-assigned number
        // on the final spec. This is the one-stage replacement for
        // the old two-stage "placeholder zeros then adapter overwrites"
        // pattern (audit finding #12).
        let mut resolved = Vec::with_capacity(guest_fds.len());
        for (guest_fd, interface) in guest_fds.into_iter().zip(&spec.resolved_interfaces) {
            let guest_raw = builder
                .inherit_fd(guest_fd)
                .context("failed to inherit vmm guest fd")?;
            resolved.push(ResolvedNetworkInterface {
                mac: interface.mac,
                guest_fd: guest_raw,
            });
        }

        let runtime_spec = VmmLaunchSpec {
            resolved_interfaces: resolved,
            ..spec.clone()
        };
        runtime_spec.validate().context("invalid vmm launch spec")?;

        let args = encode_launch_spec_args(&runtime_spec)?;
        proc::spawn_sandboxed("vmm", &binary, builder, &args, false)
            .context("failed to spawn sandboxed VMM process")
    }

    fn vmm_sandbox_builder(spec: &VmmLaunchSpec, vmm_exe: &Path) -> SandboxBuilder {
        let mut builder = capsa_sandbox::Sandbox::builder()
            .allow_network(false)
            .allow_kvm(true)
            .allow_interactive_tty(true)
            .read_only_path(canonical_or_unchanged(vmm_exe));

        if let Some(root) = &spec.root {
            builder = builder.read_write_path(canonical_or_unchanged(root));
        }
        if let Some(kernel) = &spec.kernel {
            builder = builder.read_only_path(canonical_or_unchanged(kernel));
        }
        if let Some(initramfs) = &spec.initramfs {
            builder = builder.read_only_path(canonical_or_unchanged(initramfs));
        }

        builder
    }

    /// Resolves symlinks before handing a path to the sandbox policy
    /// layer so darwin's sandbox-exec sees the same path the kernel
    /// will resolve at `open(2)` time. Falls back to the input for
    /// paths that don't exist (test fixtures, etc.).
    fn canonical_or_unchanged(path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
    }

    fn build_vmm_spec(
        config: &VmConfig,
        resolved_interfaces: Vec<ResolvedNetworkInterface>,
    ) -> VmmLaunchSpec {
        VmmLaunchSpec {
            root: config.root.clone(),
            kernel: config.kernel.clone(),
            initramfs: config.initramfs.clone(),
            kernel_cmdline: config.kernel_cmdline.clone(),
            vcpus: config.vcpus,
            memory_mib: config.memory_mib,
            verbosity: config.verbosity,
            resolved_interfaces,
        }
    }

    // ── interface helpers ────────────────────────────────────────

    fn resolve_mac(iface: &VmNetworkInterfaceConfig) -> Result<[u8; 6]> {
        match iface.mac {
            Some(mac) => {
                ensure!(mac != [0u8; 6], "interface MAC address is all zeros");
                Ok(mac)
            }
            None => Ok(generate_mac(0)),
        }
    }

    fn generate_mac(index: usize) -> [u8; 6] {
        let mut seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        seed ^= (std::process::id() as u128) << 32;
        seed ^= index as u128;

        let mut mac = [0u8; 6];
        mac[0] = 0x02; // locally administered, unicast
        mac[1] = ((seed >> 8) & 0xff) as u8;
        mac[2] = ((seed >> 16) & 0xff) as u8;
        mac[3] = ((seed >> 24) & 0xff) as u8;
        mac[4] = ((seed >> 32) & 0xff) as u8;
        mac[5] = ((seed >> 40) & 0xff) as u8;

        if mac == [0u8; 6] {
            mac[5] = 1;
        }

        mac
    }

    // ── integration tests ────────────────────────────────────────
    //
    // These tests exercise the entire VmConfig::start path through
    // real sandboxed (or bypass) spawns against shell-script fake
    // daemons. They replace the ~450 lines of generic
    // `DaemonSupervisor` tests that lived under the old
    // `daemon/supervisor.rs` module.

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::{VmConfig, VmNetworkInterfaceConfig};
        use std::path::{Path, PathBuf};
        use std::time::Instant;

        fn env_lock() -> &'static std::sync::Mutex<()> {
            crate::test_env_lock()
        }

        struct EnvVarGuard {
            key: &'static str,
            old: Option<std::ffi::OsString>,
        }

        impl EnvVarGuard {
            fn set_path(key: &'static str, value: &Path) -> Self {
                let old = std::env::var_os(key);
                std::env::set_var(key, value);
                Self { key, old }
            }

            fn set_raw(key: &'static str, value: &str) -> Self {
                let old = std::env::var_os(key);
                std::env::set_var(key, value);
                Self { key, old }
            }
        }

        impl Drop for EnvVarGuard {
            fn drop(&mut self) {
                if let Some(old) = self.old.take() {
                    std::env::set_var(self.key, old);
                } else {
                    std::env::remove_var(self.key);
                }
            }
        }

        fn make_temp_file(prefix: &str, contents: &[u8]) -> PathBuf {
            let path = unique_temp_path(prefix);
            std::fs::write(&path, contents).expect("temp file should be written");
            path
        }

        fn make_temp_executable_script(prefix: &str, body: &str) -> PathBuf {
            // Test scripts emulate netd, which receives its launch
            // spec as the argument to `--launch-spec-json`. Extract
            // `ready_fd` from that JSON so the body can reference it
            // as `$READY_FD` without hardcoding a kernel-assigned fd.
            let prelude = r#"
case "$1" in
  --launch-spec-json)
    READY_FD=$(printf '%s' "$2" | sed -n 's/.*"ready_fd":\([0-9]*\).*/\1/p')
    ;;
esac
"#;
            let path = unique_temp_path(prefix);
            std::fs::write(&path, format!("#!/bin/sh\nset -eu\n{prelude}\n{body}\n"))
                .expect("script file should be written");
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path)
                .expect("script metadata should be readable")
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).expect("script should be executable");
            path
        }

        fn unique_temp_path(prefix: &str) -> PathBuf {
            std::env::temp_dir().join(format!(
                "{prefix}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("time should be after epoch")
                    .as_nanos()
            ))
        }

        fn find_binary_in_path(name: &str) -> PathBuf {
            for dir in std::env::split_paths(&std::env::var_os("PATH").expect("PATH should be set"))
            {
                let candidate = dir.join(name);
                if candidate.exists() {
                    return candidate;
                }
            }
            panic!("binary `{name}` should be available in PATH for tests");
        }

        fn sample_config() -> VmConfig {
            VmConfig {
                root: Some("/tmp/root".into()),
                kernel: Some("/tmp/kernel".into()),
                initramfs: Some("/tmp/initramfs".into()),
                kernel_cmdline: Some("console=ttyS0".to_string()),
                vcpus: 1,
                memory_mib: 512,
                verbosity: 0,
                interfaces: vec![],
            }
        }

        fn sample_networked_config() -> VmConfig {
            let mut config = sample_config();
            config.interfaces = vec![VmNetworkInterfaceConfig {
                mac: None,
                policy: None,
                port_forwards: vec![],
            }];
            config
        }

        fn read_pid_file_with_timeout(path: &Path, timeout: Duration) -> u32 {
            let deadline = Instant::now() + timeout;
            loop {
                if let Ok(raw) = std::fs::read_to_string(path) {
                    return raw
                        .trim()
                        .parse::<u32>()
                        .expect("pid file should contain a valid pid");
                }
                if Instant::now() >= deadline {
                    panic!("pid file {} did not appear in time", path.display());
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        }

        fn process_exists(pid: u32) -> bool {
            // SAFETY: `kill(pid, 0)` does not send a signal; it is
            // only used for existence probing.
            let rc = unsafe { libc::kill(pid as i32, 0) };
            if rc == 0 {
                return true;
            }
            let err = std::io::Error::last_os_error();
            matches!(err.raw_os_error(), Some(libc::EPERM))
        }

        fn wait_for_process_exit(pid: u32, timeout: Duration) {
            let deadline = Instant::now() + timeout;
            while Instant::now() < deadline {
                if !process_exists(pid) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            panic!("process {pid} should have exited within {timeout:?}");
        }

        // ── validation path ──────────────────────────────────────

        #[test]
        fn start_rejects_multiple_interfaces_before_spawning_anything() {
            let mut config = sample_config();
            config.interfaces = vec![
                VmNetworkInterfaceConfig {
                    mac: None,
                    policy: None,
                    port_forwards: vec![],
                },
                VmNetworkInterfaceConfig {
                    mac: None,
                    policy: None,
                    port_forwards: vec![],
                },
            ];

            let err = config.start().expect_err("start should fail validation");
            let rendered = format!("{err:#}");
            assert!(rendered.contains("invalid VM configuration"));
            assert!(rendered.contains("multiple network interfaces are not supported yet"));
        }

        // ── no-network path ──────────────────────────────────────

        #[test]
        fn no_network_start_succeeds_when_vmm_exits_zero() {
            let _env_lock = env_lock()
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let vmm_true = find_binary_in_path("true");
            let _vmm_guard = EnvVarGuard::set_path("CAPSA_VMM_PATH", &vmm_true);
            let _sandbox_guard = EnvVarGuard::set_raw("CAPSA_DISABLE_SANDBOX", "1");

            sample_config()
                .start()
                .expect("no-network start should succeed for zero exit");
        }

        #[test]
        fn no_network_start_does_not_require_netd_binary() {
            let _env_lock = env_lock()
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let vmm_true = find_binary_in_path("true");
            let _vmm_guard = EnvVarGuard::set_path("CAPSA_VMM_PATH", &vmm_true);
            let _netd_guard =
                EnvVarGuard::set_path("CAPSA_NETD_PATH", Path::new("/definitely/missing/netd"));
            let _sandbox_guard = EnvVarGuard::set_raw("CAPSA_DISABLE_SANDBOX", "1");

            sample_config()
                .start()
                .expect("no-network path should not try to spawn netd");
        }

        #[test]
        fn no_network_start_reports_vmm_spawn_failure() {
            let _env_lock = env_lock()
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let non_executable = make_temp_file("capsa-vmm-non-executable", b"not executable");
            let _vmm_guard = EnvVarGuard::set_path("CAPSA_VMM_PATH", &non_executable);
            let _sandbox_guard = EnvVarGuard::set_raw("CAPSA_DISABLE_SANDBOX", "1");

            let err = sample_config()
                .start()
                .expect_err("no-network start should fail when VMM cannot be spawned");

            let _ = std::fs::remove_file(non_executable);

            // resolve_binary now catches non-executable files up
            // front, so the error points at resolution rather than
            // `Command::spawn`.
            let rendered = format!("{err:#}");
            assert!(
                rendered.contains("failed to resolve VMM binary"),
                "unexpected: {rendered}"
            );
        }

        #[test]
        fn no_network_start_propagates_vmm_non_zero_exit_status() {
            let _env_lock = env_lock()
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let vmm_false = find_binary_in_path("false");
            let _vmm_guard = EnvVarGuard::set_path("CAPSA_VMM_PATH", &vmm_false);
            let _sandbox_guard = EnvVarGuard::set_raw("CAPSA_DISABLE_SANDBOX", "1");

            let err = sample_config()
                .start()
                .expect_err("no-network start should fail for non-zero VMM exit");
            let rendered = format!("{err:#}");
            assert!(rendered.contains("sandboxed VMM process exited with status"));
        }

        // ── network path ─────────────────────────────────────────

        #[test]
        fn network_start_aborts_when_netd_readiness_times_out() {
            let _env_lock = env_lock()
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let netd =
                make_temp_executable_script("capsa-netd-timeout", "while true; do sleep 1; done");
            let vmm = find_binary_in_path("true");
            let _netd_guard = EnvVarGuard::set_path("CAPSA_NETD_PATH", &netd);
            let _vmm_guard = EnvVarGuard::set_path("CAPSA_VMM_PATH", &vmm);
            let _sandbox_guard = EnvVarGuard::set_raw("CAPSA_DISABLE_SANDBOX", "1");

            let err = sample_networked_config()
                .start()
                .expect_err("startup should fail if netd does not signal readiness");

            let _ = std::fs::remove_file(netd);

            assert!(
                format!("{err:#}").contains("timed out waiting for net daemon readiness signal")
            );
        }

        #[test]
        fn network_start_vmm_spawn_failure_tears_down_netd() {
            let _env_lock = env_lock()
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let netd_pid_file = unique_temp_path("capsa-netd-pid");
            let netd = make_temp_executable_script(
                "capsa-netd-ready-loop",
                &format!(
                    "echo $$ > '{}'\neval \"printf 'R' >&${{READY_FD}}\"\nwhile true; do sleep 1; done",
                    netd_pid_file.display()
                ),
            );
            let non_executable_vmm = make_temp_file("capsa-vmm-non-executable", b"not executable");

            let _netd_guard = EnvVarGuard::set_path("CAPSA_NETD_PATH", &netd);
            let _vmm_guard = EnvVarGuard::set_path("CAPSA_VMM_PATH", &non_executable_vmm);
            let _sandbox_guard = EnvVarGuard::set_raw("CAPSA_DISABLE_SANDBOX", "1");

            let err = sample_networked_config()
                .start()
                .expect_err("VMM spawn failure should fail startup");

            let netd_pid = read_pid_file_with_timeout(&netd_pid_file, Duration::from_secs(2));
            wait_for_process_exit(netd_pid, Duration::from_secs(4));

            let _ = std::fs::remove_file(netd_pid_file);
            let _ = std::fs::remove_file(netd);
            let _ = std::fs::remove_file(non_executable_vmm);

            let rendered = format!("{err:#}");
            assert!(
                rendered.contains("failed to spawn sandboxed VMM process")
                    || rendered.contains("failed to resolve VMM binary"),
                "unexpected: {rendered}"
            );
        }

        #[test]
        fn network_start_netd_runtime_exit_tears_down_vmm() {
            let _env_lock = env_lock()
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let vmm_pid_file = unique_temp_path("capsa-vmm-pid");
            let vmm = make_temp_executable_script(
                "capsa-vmm-loop",
                &format!(
                    "echo $$ > '{}'\nwhile true; do sleep 1; done",
                    vmm_pid_file.display()
                ),
            );
            let netd = make_temp_executable_script(
                "capsa-netd-ready-then-exit",
                "eval \"printf 'R' >&${READY_FD}\"\nsleep 0.2\nexit 42",
            );

            let _netd_guard = EnvVarGuard::set_path("CAPSA_NETD_PATH", &netd);
            let _vmm_guard = EnvVarGuard::set_path("CAPSA_VMM_PATH", &vmm);
            let _sandbox_guard = EnvVarGuard::set_raw("CAPSA_DISABLE_SANDBOX", "1");

            let err = sample_networked_config()
                .start()
                .expect_err("net daemon runtime exit should fail launcher");

            let vmm_pid = read_pid_file_with_timeout(&vmm_pid_file, Duration::from_secs(2));
            wait_for_process_exit(vmm_pid, Duration::from_secs(4));

            let _ = std::fs::remove_file(vmm_pid_file);
            let _ = std::fs::remove_file(vmm);
            let _ = std::fs::remove_file(netd);

            assert!(format!("{err:#}").contains("network daemon exited unexpectedly"));
        }

        #[test]
        fn network_start_vmm_exit_tears_down_netd() {
            let _env_lock = env_lock()
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            let netd_pid_file = unique_temp_path("capsa-netd-pid");
            let netd = make_temp_executable_script(
                "capsa-netd-ready-loop",
                &format!(
                    "echo $$ > '{}'\neval \"printf 'R' >&${{READY_FD}}\"\nwhile true; do sleep 1; done",
                    netd_pid_file.display()
                ),
            );
            let vmm = find_binary_in_path("true");

            let _netd_guard = EnvVarGuard::set_path("CAPSA_NETD_PATH", &netd);
            let _vmm_guard = EnvVarGuard::set_path("CAPSA_VMM_PATH", &vmm);
            let _sandbox_guard = EnvVarGuard::set_raw("CAPSA_DISABLE_SANDBOX", "1");

            sample_networked_config()
                .start()
                .expect("VMM zero exit should propagate success");

            let netd_pid = read_pid_file_with_timeout(&netd_pid_file, Duration::from_secs(2));
            wait_for_process_exit(netd_pid, Duration::from_secs(4));

            let _ = std::fs::remove_file(netd_pid_file);
            let _ = std::fs::remove_file(netd);
        }

        // ── small unit tests for helpers ─────────────────────────

        #[test]
        fn generated_mac_is_non_zero() {
            assert_ne!(generate_mac(0), [0u8; 6]);
        }

        #[test]
        fn resolve_mac_rejects_explicit_zero() {
            let iface = VmNetworkInterfaceConfig {
                mac: Some([0; 6]),
                policy: None,
                port_forwards: vec![],
            };
            let err = resolve_mac(&iface).expect_err("zero mac should be rejected");
            assert!(err.to_string().contains("MAC address is all zeros"));
        }

        #[test]
        fn resolve_mac_passes_through_explicit_nonzero() {
            let explicit = [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee];
            let iface = VmNetworkInterfaceConfig {
                mac: Some(explicit),
                policy: None,
                port_forwards: vec![],
            };
            assert_eq!(resolve_mac(&iface).unwrap(), explicit);
        }
    }
}
