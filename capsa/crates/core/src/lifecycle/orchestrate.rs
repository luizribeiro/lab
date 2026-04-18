//! High-level VM launch flow. Splits the single-VM lifecycle into a
//! spawn phase and a wait phase so callers can hold a handle to a
//! running VM.

use std::os::fd::OwnedFd;

use anyhow::{bail, Context, Result};

use crate::config::VmConfig;

use super::child::{self, ChildHandle, Exited};
use super::netd::{self, NetdSpawn};
use super::plan::{self, VmmInterfaceBinding};
use super::vmm;

/// A resolved VM-side attachment produced by the caller (e.g. the
/// public `capsa` crate after sending `AddInterface` to an external
/// netd). The `guest_fd` will be inherited into the sandboxed vmm.
pub struct VmAttachment {
    pub mac: [u8; 6],
    pub guest_fd: OwnedFd,
}

pub struct VmProcesses {
    vmm: ChildHandle,
    netd: Option<ChildHandle>,
}

impl VmProcesses {
    /// Spawn a vmm whose interfaces are already attached to an
    /// external network daemon. The caller has already sent
    /// `AddInterface` over the daemon's control socket for each
    /// attachment and now hands the guest-side fds to this function.
    /// No internal netd is started; the returned `VmProcesses`
    /// supervises only the vmm child.
    pub fn spawn_with_attachments(
        config: &VmConfig,
        attachments: Vec<VmAttachment>,
    ) -> Result<Self> {
        config.validate().context("invalid VM configuration")?;
        let bindings: Vec<VmmInterfaceBinding> = attachments
            .into_iter()
            .map(|a| VmmInterfaceBinding {
                mac: a.mac,
                guest_fd: a.guest_fd,
            })
            .collect();
        let vmm =
            vmm::spawn_vmm(config, bindings).context("failed to spawn sandboxed VMM process")?;
        Ok(Self { vmm, netd: None })
    }

    pub(super) fn spawn(config: &VmConfig) -> Result<Self> {
        config.validate().context("invalid VM configuration")?;

        if config.interfaces.is_empty() {
            let vmm = vmm::spawn_vmm(config, vec![])?;
            return Ok(Self { vmm, netd: None });
        }

        let plans = plan::plan_interfaces(config)?;
        let sockets = plan::open_interface_sockets(plans)?;

        let (
            NetdSpawn {
                child: netd_child,
                ready_reader,
            },
            attachment,
            bindings,
        ) = netd::spawn_netd(
            sockets,
            config.interfaces.first().and_then(|i| i.policy.clone()),
        )?;

        netd::wait_ready(ready_reader, netd::READINESS_TIMEOUT)
            .context("netd readiness check failed")?;

        attachment
            .attach_all()
            .context("failed to attach VM interfaces via netd control socket")?;

        // If spawn_vmm errors below, `netd_child` is dropped here and
        // its `ChildHandle::Drop` tears down the netd child. No
        // explicit cleanup needed — this is the whole point of RAII.
        let vmm =
            vmm::spawn_vmm(config, bindings).context("failed to spawn sandboxed VMM process")?;

        Ok(Self {
            vmm,
            netd: Some(netd_child),
        })
    }

    pub fn wait(&mut self) -> Result<()> {
        let Some(netd) = self.netd.as_mut() else {
            let status = self
                .vmm
                .wait_by_ref()
                .context("failed to wait on sandboxed VMM child")?;
            return if status.success() {
                Ok(())
            } else {
                bail!("sandboxed VMM process exited with status {status}")
            };
        };

        match child::wait_either(&mut self.vmm, netd) {
            Exited::First(Ok(status)) if status.success() => Ok(()),
            Exited::First(Ok(status)) => {
                bail!("sandboxed VMM process exited with status {status}")
            }
            Exited::First(Err(err)) => Err(err).context("failed to reap VMM process"),
            Exited::Second(Ok(status)) => {
                bail!(
                    "network daemon exited unexpectedly while VMM was running with status {status}"
                )
            }
            Exited::Second(Err(err)) => Err(err).context("failed to reap network daemon"),
        }
    }
}

impl VmProcesses {
    /// SIGKILL both child processes and wait for their reapers to
    /// publish exit status. Safe to call after the children have
    /// already exited on their own (becomes a no-op). Also invoked
    /// implicitly by `Drop`.
    pub fn kill(&mut self) -> std::io::Result<()> {
        if let Some(netd) = self.netd.as_mut() {
            netd.kill()?;
        }
        self.vmm.kill()
    }
}

impl Drop for VmProcesses {
    fn drop(&mut self) {
        if let Err(err) = self.kill() {
            tracing::warn!(error = %err, "drop-time SIGKILL failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lifecycle::test_helpers::{
        env_lock, fake_netd_path, find_binary_on_path, unique_temp_path, EnvVarGuard,
    };
    use crate::lifecycle::NetworkProcesses;
    use crate::VmConfig;
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixDatagram;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use super::{VmAttachment, VmProcesses};

    fn make_temp_file(prefix: &str, contents: &[u8]) -> PathBuf {
        let path = unique_temp_path(prefix);
        std::fs::write(&path, contents).expect("temp file should be written");
        path
    }

    fn make_temp_executable_script(prefix: &str, body: &str) -> PathBuf {
        let path = unique_temp_path(prefix);
        std::fs::write(&path, format!("#!/bin/sh\nset -eu\n{body}\n"))
            .expect("script file should be written");
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)
            .expect("script metadata should be readable")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).expect("script should be executable");
        path
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

    /// Create a host/guest socketpair and return the guest-side
    /// `VmAttachment` plus the host-side fd (to be handed to
    /// `NetworkProcesses::attach`).
    fn make_attachment(mac: [u8; 6]) -> (VmAttachment, OwnedFd) {
        let (host, guest) = UnixDatagram::pair().expect("socketpair");
        (
            VmAttachment {
                mac,
                guest_fd: guest.into(),
            },
            host.into(),
        )
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

    // ── no-network path (spawn_with_attachments, empty) ──────

    #[test]
    fn spawn_with_empty_attachments_succeeds_when_vmm_exits_zero() {
        let _env_lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let vmm_true = find_binary_on_path("true");
        let _vmm_guard = EnvVarGuard::set_path("CAPSA_VMM_PATH", &vmm_true);
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");

        let mut processes = VmProcesses::spawn_with_attachments(&sample_config(), vec![])
            .expect("spawn_with_attachments should succeed");
        processes.wait().expect("wait should succeed for exit 0");
    }

    #[test]
    fn spawn_with_empty_attachments_does_not_require_netd_binary() {
        let _env_lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let vmm_true = find_binary_on_path("true");
        let _vmm_guard = EnvVarGuard::set_path("CAPSA_VMM_PATH", &vmm_true);
        let _netd_guard =
            EnvVarGuard::set_path("CAPSA_NETD_PATH", Path::new("/definitely/missing/netd"));
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");

        let mut processes = VmProcesses::spawn_with_attachments(&sample_config(), vec![])
            .expect("no-network path should not try to spawn netd");
        processes.wait().expect("wait should succeed");
    }

    #[test]
    fn spawn_with_empty_attachments_reports_vmm_spawn_failure() {
        let _env_lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let non_executable = make_temp_file("capsa-vmm-non-executable", b"not executable");
        let _vmm_guard = EnvVarGuard::set_path("CAPSA_VMM_PATH", &non_executable);
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");

        let err = match VmProcesses::spawn_with_attachments(&sample_config(), vec![]) {
            Ok(_) => panic!("spawn should fail when VMM cannot be resolved"),
            Err(err) => err,
        };

        let _ = std::fs::remove_file(non_executable);

        let rendered = format!("{err:#}");
        assert!(
            rendered.contains("failed to resolve VMM binary"),
            "unexpected: {rendered}"
        );
    }

    #[test]
    fn spawn_with_empty_attachments_propagates_vmm_non_zero_exit_status() {
        let _env_lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let vmm_false = find_binary_on_path("false");
        let _vmm_guard = EnvVarGuard::set_path("CAPSA_VMM_PATH", &vmm_false);
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");

        let mut processes = VmProcesses::spawn_with_attachments(&sample_config(), vec![])
            .expect("spawn should succeed");
        let err = processes.wait().expect_err("non-zero exit should fail");
        assert!(format!("{err:#}").contains("sandboxed VMM process exited with status"));
    }

    // ── networked path (external NetworkProcesses) ──────────

    #[test]
    fn spawn_with_external_network_succeeds_end_to_end() {
        let _env_lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _netd_guard = EnvVarGuard::set_path("CAPSA_NETD_PATH", &fake_netd_path());
        let _vmm_guard = EnvVarGuard::set_path("CAPSA_VMM_PATH", &find_binary_on_path("true"));
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");

        let network = NetworkProcesses::spawn(None).expect("spawn fake netd");
        let mac = [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee];
        let (attachment, host_fd) = make_attachment(mac);
        network
            .attach(mac, vec![], &host_fd)
            .expect("attach interface via control socket");

        let mut processes = VmProcesses::spawn_with_attachments(&sample_config(), vec![attachment])
            .expect("spawn with attachment");
        processes.wait().expect("wait should succeed for exit 0");
    }

    #[test]
    fn dropping_vm_processes_sigkills_vmm_without_sigterm_grace() {
        let _env_lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let vmm_pid_file = unique_temp_path("capsa-vmm-sigkill-pid");
        // Trap SIGTERM so only SIGKILL can terminate this process.
        let vmm = make_temp_executable_script(
            "capsa-vmm-sigterm-trap",
            &format!(
                "trap '' TERM\necho $$ > '{}'\nwhile true; do sleep 1; done",
                vmm_pid_file.display()
            ),
        );
        let _vmm_guard = EnvVarGuard::set_path("CAPSA_VMM_PATH", &vmm);
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");

        let processes = VmProcesses::spawn_with_attachments(&sample_config(), vec![])
            .expect("spawn should succeed");
        let pid = read_pid_file_with_timeout(&vmm_pid_file, Duration::from_secs(2));

        let started = Instant::now();
        drop(processes);
        let elapsed = started.elapsed();

        wait_for_process_exit(pid, Duration::from_secs(2));

        let _ = std::fs::remove_file(&vmm_pid_file);
        let _ = std::fs::remove_file(&vmm);

        assert!(
            elapsed < Duration::from_secs(1),
            "drop should SIGKILL immediately, not wait the 2s SIGTERM grace; elapsed = {elapsed:?}"
        );
    }
}
