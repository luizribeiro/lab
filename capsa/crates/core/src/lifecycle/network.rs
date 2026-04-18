//! Standalone network lifecycle — spawns a `capsa-netd` daemon that
//! can accept interface attachments from many VMs. Used by the
//! public `capsa` crate to back a user-facing `NetworkHandle`.

use std::os::fd::{AsRawFd, OwnedFd};
use std::sync::Mutex;

use anyhow::{Context, Result};
use capsa_net::NetworkPolicy;
use capsa_spec::{encode_launch_spec_args, NetLaunchSpec};
use nix::sys::socket::{socketpair, AddressFamily, SockFlag, SockType};

use super::child::{self, ChildHandle};
use super::control_client::ControlClient;
use super::netd::{netd_sandbox_builder, wait_ready, READINESS_TIMEOUT};

pub struct NetworkProcesses {
    // Held for its lifetime; dropping triggers kill_on_drop SIGKILL.
    #[allow(dead_code)]
    netd: ChildHandle,
    control: Mutex<ControlClient>,
}

impl NetworkProcesses {
    /// Spawn a `capsa-netd` daemon and await its readiness
    /// handshake. The returned handle can accept interface
    /// attachments via [`NetworkProcesses::attach`].
    pub async fn spawn(policy: Option<NetworkPolicy>) -> Result<Self> {
        let binary = child::resolve_binary("CAPSA_NETD_PATH", "capsa-netd")
            .context("failed to resolve net daemon binary")?;
        let (ready_reader, ready_writer) =
            std::io::pipe().context("failed to create netd readiness pipe")?;

        let (client_sock, netd_sock) = socketpair(
            AddressFamily::Unix,
            SockType::SeqPacket,
            None,
            SockFlag::SOCK_CLOEXEC,
        )
        .context("failed to create netd control socketpair")?;

        let builder = netd_sandbox_builder(&binary);

        let ready_writer_owned = OwnedFd::from(ready_writer);
        let ready_fd_num = ready_writer_owned.as_raw_fd();
        let control_fd_num = netd_sock.as_raw_fd();
        let fds: Vec<OwnedFd> = vec![ready_writer_owned, netd_sock];

        let spec = NetLaunchSpec {
            ready_fd: ready_fd_num,
            control_fd: Some(control_fd_num),
            policy,
        };
        spec.validate().context("invalid netd launch spec")?;

        let args = encode_launch_spec_args(&spec)?;
        let netd = child::spawn_sandboxed("netd", &binary, builder, fds, &args, true)
            .context("failed to spawn network daemon")?;

        wait_ready(OwnedFd::from(ready_reader), READINESS_TIMEOUT)
            .await
            .context("netd readiness check failed")?;

        Ok(Self {
            netd,
            control: Mutex::new(ControlClient::new(client_sock)),
        })
    }

    /// Send an `AddInterface` request over the control socket.
    /// `host_fd` is the host-side end of the guest socketpair;
    /// it will be duplicated into netd via `SCM_RIGHTS`.
    pub fn attach(
        &self,
        mac: [u8; 6],
        port_forwards: Vec<(u16, u16)>,
        udp_forwards: Vec<(u16, u16)>,
        host_fd: &OwnedFd,
    ) -> Result<()> {
        self.control
            .lock()
            .expect("control mutex poisoned")
            .send_add_interface(mac, port_forwards, udp_forwards, host_fd)
    }
}

// No explicit Drop: the underlying `tokio::process::Child` is
// spawned with `kill_on_drop(true)`, so dropping a `ChildHandle`
// (and therefore a `NetworkProcesses`) sends SIGKILL automatically.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::test_helpers::{env_lock, fake_netd_path, unique_temp_path, EnvVarGuard};
    use std::path::Path;
    use std::time::{Duration, Instant};

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

    async fn wait_for_process_exit(pid: u32, timeout: Duration) {
        let deadline = tokio::time::Instant::now() + timeout;
        while tokio::time::Instant::now() < deadline {
            if !process_exists(pid) {
                return;
            }
            // Async sleep: yields to the tokio runtime so the
            // pidfd-backed child reaper can drive SIGKILL delivery
            // and waitpid while we poll.
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("process {pid} should have exited within {timeout:?}");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn spawn_fails_when_netd_readiness_times_out() {
        let _env_lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let _netd_guard = EnvVarGuard::set_path("CAPSA_NETD_PATH", &fake_netd_path());
        let _skip_ready = EnvVarGuard::set("FAKE_NETD_SKIP_READY", "1");
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");

        let err = match NetworkProcesses::spawn(None).await {
            Ok(_) => panic!("spawn should fail without readiness signal"),
            Err(err) => err,
        };
        assert!(format!("{err:#}").contains("timed out waiting for net daemon readiness signal"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dropping_network_processes_sigkills_netd() {
        let _env_lock = env_lock()
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let netd_pid_file = unique_temp_path("capsa-netd-drop-pid");
        let _netd_guard = EnvVarGuard::set_path("CAPSA_NETD_PATH", &fake_netd_path());
        let _pid_file_guard = EnvVarGuard::set_path("FAKE_NETD_PID_FILE", &netd_pid_file);
        // Trap SIGTERM so only SIGKILL can terminate the fake netd.
        let _trap_guard = EnvVarGuard::set("FAKE_NETD_TRAP_SIGTERM", "1");
        let _sandbox_guard = EnvVarGuard::set("CAPSA_DISABLE_SANDBOX", "1");

        let network = NetworkProcesses::spawn(None).await.expect("spawn netd");
        let pid = read_pid_file_with_timeout(&netd_pid_file, Duration::from_secs(2));

        let started = Instant::now();
        drop(network);
        let elapsed = started.elapsed();

        wait_for_process_exit(pid, Duration::from_secs(2)).await;

        let _ = std::fs::remove_file(&netd_pid_file);

        assert!(
            elapsed < Duration::from_secs(1),
            "drop should SIGKILL immediately; elapsed = {elapsed:?}"
        );
    }
}
