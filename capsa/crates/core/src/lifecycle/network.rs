//! Standalone network lifecycle — spawns a `capsa-netd` daemon that
//! can accept interface attachments from many VMs. Used by the
//! public `capsa` crate to back a user-facing `NetworkHandle`.

use std::os::fd::{AsRawFd, OwnedFd};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use anyhow::{Context, Result};
use capsa_net::NetworkPolicy;
use capsa_spec::{encode_launch_spec_args, NetLaunchSpec};
use lockin::SandboxBuilder;
use nix::sys::socket::{socketpair, AddressFamily, SockFlag, SockType};

use super::child::{self, ChildHandle};
use super::control_client::ControlClient;
use super::netd::{wait_ready, READINESS_TIMEOUT};
use super::plan;

pub struct NetworkProcesses {
    netd: Mutex<Option<ChildHandle>>,
    control: Mutex<ControlClient>,
}

impl NetworkProcesses {
    /// Spawn a `capsa-netd` daemon and wait for it to signal
    /// readiness. The returned handle can accept interface
    /// attachments via [`NetworkProcesses::attach`].
    pub fn spawn(policy: Option<NetworkPolicy>) -> Result<Self> {
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
            .context("netd readiness check failed")?;

        Ok(Self {
            netd: Mutex::new(Some(netd)),
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
        host_fd: &OwnedFd,
    ) -> Result<()> {
        self.control
            .lock()
            .expect("control mutex poisoned")
            .send_add_interface(mac, port_forwards, host_fd)
    }
}

impl Drop for NetworkProcesses {
    fn drop(&mut self) {
        if let Some(mut netd) = self.netd.lock().ok().and_then(|mut g| g.take()) {
            if let Err(err) = netd.kill() {
                tracing::warn!(error = %err, "drop-time SIGKILL of netd failed");
            }
        }
    }
}

fn netd_sandbox_builder(binary_path: &Path) -> SandboxBuilder {
    let mut builder = lockin::Sandbox::builder()
        .allow_network(true)
        .read_only_path(plan::canonical_or_unchanged(binary_path));
    builder = child::apply_syd_path(builder);
    builder = child::apply_library_dirs(builder);
    for runtime_read_path in capsa_net::runtime_read_paths() {
        builder = builder.read_only_path(PathBuf::from(*runtime_read_path));
    }
    builder
}
