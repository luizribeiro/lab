//! High-level VM launch flow. Decides whether to run with or
//! without networking, threads each interface through the typed
//! phases in `plan`, and supervises the pair until one exits.

use anyhow::{bail, Context, Result};

use crate::config::VmConfig;

use super::child::{self, Exited};
use super::netd::{self, NetdSpawn};
use super::plan;
use super::vmm;

pub(super) fn run(config: &VmConfig) -> Result<()> {
    config.validate().context("invalid VM configuration")?;

    if config.interfaces.is_empty() {
        return run_vmm_only(config);
    }

    run_with_network(config)
}

fn run_vmm_only(config: &VmConfig) -> Result<()> {
    let vmm_child = vmm::spawn_vmm(config, vec![])?;
    let status = vmm_child
        .wait()
        .context("failed to wait on sandboxed VMM child")?;
    if status.success() {
        Ok(())
    } else {
        bail!("sandboxed VMM process exited with status {status}")
    }
}

fn run_with_network(config: &VmConfig) -> Result<()> {
    let plans = plan::plan_interfaces(config)?;
    let sockets = plan::open_interface_sockets(plans)?;

    let (
        NetdSpawn {
            child: mut netd_child,
            ready_reader,
        },
        bindings,
    ) = netd::spawn_netd(sockets)?;

    netd::wait_ready(ready_reader, netd::READINESS_TIMEOUT)
        .context("netd readiness check failed")?;

    // If spawn_vmm errors below, `netd_child` is dropped here and
    // its `ChildHandle::Drop` tears down the netd child. No
    // explicit cleanup needed — this is the whole point of RAII.
    let mut vmm_child =
        vmm::spawn_vmm(config, bindings).context("failed to spawn sandboxed VMM process")?;

    match child::wait_either(&mut vmm_child, &mut netd_child) {
        Exited::First(Ok(status)) if status.success() => Ok(()),
        Exited::First(Ok(status)) => {
            bail!("sandboxed VMM process exited with status {status}")
        }
        Exited::First(Err(err)) => Err(err).context("failed to reap VMM process"),
        Exited::Second(Ok(status)) => {
            bail!("network daemon exited unexpectedly while VMM was running with status {status}")
        }
        Exited::Second(Err(err)) => Err(err).context("failed to reap network daemon"),
    }
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
    // Tests drive the public `VmConfig::start` entry point through
    // bypass-mode spawns; nothing in this `tests` module needs the
    // private orchestration helpers.
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
}
