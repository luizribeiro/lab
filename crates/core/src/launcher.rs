mod interface_plan;

use std::os::fd::OwnedFd;
use std::time::Duration;

use anyhow::{bail, Context, Result};

use crate::{
    daemon::{
        net::{
            adapter::{NetDaemonAdapter, NetDaemonHandoff},
            spec::{NetInterfaceSpec, NetLaunchSpec},
        },
        supervisor::DaemonSupervisor,
        vmm::{
            adapter::{VmmDaemonAdapter, VmmDaemonHandoff},
            spec::VmmLaunchSpec,
        },
    },
    VmConfig,
};

use self::interface_plan::{build_interface_plan, resolved_interfaces_for_plan};

const MONITOR_POLL_INTERVAL: Duration = Duration::from_millis(50);

impl VmConfig {
    /// Start the VM in sandboxed daemon processes.
    pub fn start(&self) -> Result<()> {
        self.validate().context("invalid VM configuration")?;

        if self.interfaces.is_empty() {
            return start_without_network_via_supervisor(self);
        }

        start_with_network_via_supervisor(self)
    }
}

fn start_without_network_via_supervisor(config: &VmConfig) -> Result<()> {
    let launch_spec = VmmLaunchSpec {
        vm_config: config.clone(),
        resolved_interfaces: vec![],
    };
    let handoff = VmmDaemonHandoff::new(vec![]).context("failed to prepare VMM handoff")?;

    let supervisor = DaemonSupervisor::default();
    let mut vmm = supervisor
        .spawn::<VmmDaemonAdapter>(launch_spec, handoff)
        .context("failed to spawn sandboxed VMM process")?;

    let status = vmm
        .wait_blocking()
        .context("failed to wait on sandboxed child")?;
    if status.success() {
        return Ok(());
    }

    bail!("sandboxed VMM process exited with status {status}")
}

fn start_with_network_via_supervisor(config: &VmConfig) -> Result<()> {
    let plan = build_interface_plan(&config.interfaces)
        .context("failed to build network interface plan for daemon startup")?;

    let vmm_launch_spec = VmmLaunchSpec {
        vm_config: config.clone(),
        resolved_interfaces: resolved_interfaces_for_plan(&plan.interfaces),
    };

    let net_launch_spec = NetLaunchSpec {
        interfaces: plan
            .interfaces
            .iter()
            .map(|iface| NetInterfaceSpec {
                host_fd: iface.netd_host_target_fd,
                mac: iface.mac,
                policy: Some(iface.policy.clone()),
            })
            .collect(),
        port_forwards: config
            .interfaces
            .first()
            .map(|iface| iface.port_forwards.clone())
            .unwrap_or_default(),
    };

    let mut host_fds = Vec::with_capacity(plan.interfaces.len());
    let mut guest_fds = Vec::with_capacity(plan.interfaces.len());
    for iface in plan.interfaces {
        host_fds.push(iface.host_fd);
        guest_fds.push(iface.guest_fd);
    }

    let (ready_reader, ready_writer) =
        create_pipe().context("failed to create netd readiness pipe")?;
    let net_handoff = NetDaemonHandoff::new(host_fds, ready_reader, ready_writer)
        .context("failed to prepare net daemon handoff")?;
    let vmm_handoff = VmmDaemonHandoff::new(guest_fds).context("failed to prepare VMM handoff")?;

    let supervisor = DaemonSupervisor::default();

    let mut netd = supervisor
        .spawn::<NetDaemonAdapter>(net_launch_spec, net_handoff)
        .context("failed to spawn network daemon")?;

    let mut vmm = match supervisor.spawn::<VmmDaemonAdapter>(vmm_launch_spec, vmm_handoff) {
        Ok(vmm) => vmm,
        Err(primary_error) => {
            let mut error = primary_error.context("failed to spawn sandboxed VMM process");
            if let Err(cleanup_error) = netd.shutdown() {
                error = attach_cleanup_error(
                    error,
                    cleanup_error
                        .context("failed to shutdown network daemon after VMM spawn failure"),
                );
            }
            return Err(error);
        }
    };

    loop {
        if let Some(status) = vmm.try_wait().context("failed to poll VMM daemon")? {
            let shutdown_result = netd.shutdown();
            if status.success() {
                return shutdown_result.context("failed to shutdown network daemon after VMM exit");
            }

            let mut error = anyhow::anyhow!("sandboxed VMM process exited with status {status}");
            if let Err(cleanup_error) = shutdown_result {
                error = attach_cleanup_error(
                    error,
                    cleanup_error.context("failed to shutdown network daemon after VMM exit"),
                );
            }
            return Err(error);
        }

        if let Some(status) = netd.try_wait().context("failed to poll network daemon")? {
            let mut error = anyhow::anyhow!(
                "network daemon exited unexpectedly while VMM was running with status {status}"
            );
            if let Err(cleanup_error) = vmm.shutdown() {
                error = attach_cleanup_error(
                    error,
                    cleanup_error.context("failed to shutdown VMM after network daemon exit"),
                );
            }
            return Err(error);
        }

        std::thread::sleep(MONITOR_POLL_INTERVAL);
    }
}

fn create_pipe() -> Result<(OwnedFd, OwnedFd)> {
    // Use `std::io::pipe()` (stable since Rust 1.87) which creates the pipe
    // with `O_CLOEXEC` set on both ends. The previous `libc::pipe()` call
    // left them as inheritable, so any child the parent spawned between
    // pipe creation and the intended `Command::spawn` would inherit these
    // readiness fds unintentionally.
    let (reader, writer) = std::io::pipe().context("failed to create readiness pipe")?;
    Ok((reader.into(), writer.into()))
}

fn attach_cleanup_error(primary: anyhow::Error, cleanup: anyhow::Error) -> anyhow::Error {
    primary.context(format!("cleanup error: {cleanup:#}"))
}

#[cfg(test)]
mod tests {
    use super::MONITOR_POLL_INTERVAL;
    use crate::{VmConfig, VmNetworkInterfaceConfig};
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

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
        let path = unique_temp_path(prefix);
        std::fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n"))
            .expect("script file should be written");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&path)
                .expect("script metadata should be readable")
                .permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&path, perms).expect("script should be executable");
        }
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
        for dir in std::env::split_paths(&std::env::var_os("PATH").expect("PATH should be set")) {
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
            std::thread::sleep(MONITOR_POLL_INTERVAL);
        }
    }

    fn process_exists(pid: u32) -> bool {
        // SAFETY: `kill(pid, 0)` does not send a signal and is used purely for existence checks.
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
            std::thread::sleep(MONITOR_POLL_INTERVAL);
        }
        panic!("process {pid} should have exited within {timeout:?}");
    }

    #[test]
    fn start_rejects_multiple_interfaces_before_spawning_sidecar() {
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

    #[test]
    fn no_network_start_via_supervisor_succeeds_when_vmm_exits_zero() {
        let _env_lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let vmm_true = find_binary_in_path("true");
        let _vmm_guard = EnvVarGuard::set_path("CAPSA_VMM_PATH", &vmm_true);
        let _sandbox_guard = EnvVarGuard::set_raw("CAPSA_DISABLE_SANDBOX", "1");

        let config = sample_config();
        config
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

        let config = sample_config();
        let err = config
            .start()
            .expect_err("no-network start should fail when VMM cannot be spawned");

        let _ = std::fs::remove_file(non_executable);

        assert!(format!("{err:#}").contains("failed to spawn sandboxed VMM process"));
    }

    #[test]
    fn no_network_start_propagates_vmm_non_zero_exit_status() {
        let _env_lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let vmm_false = find_binary_in_path("false");
        let _vmm_guard = EnvVarGuard::set_path("CAPSA_VMM_PATH", &vmm_false);
        let _sandbox_guard = EnvVarGuard::set_raw("CAPSA_DISABLE_SANDBOX", "1");

        let config = sample_config();
        let err = config
            .start()
            .expect_err("no-network start should fail for non-zero VMM exit");
        let rendered = format!("{err:#}");

        assert!(rendered.contains("sandboxed VMM process exited with status"));
    }

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

        assert!(format!("{err:#}").contains("timed out waiting for net daemon readiness signal"));
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
                "echo $$ > '{}'\nprintf 'R' >&30\nwhile true; do sleep 1; done",
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

        assert!(format!("{err:#}").contains("failed to spawn sandboxed VMM process"));
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
            "printf 'R' >&30\nsleep 0.2\nexit 42",
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
                "echo $$ > '{}'\nprintf 'R' >&30\nwhile true; do sleep 1; done",
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
}
