use std::io::Read;
use std::os::fd::{AsRawFd, OwnedFd};
use std::path::Path;
use std::time::Duration;

use anyhow::{bail, ensure, Context, Result};

use crate::daemon::traits::{DaemonAdapter, DaemonBinaryInfo, DaemonReadiness, DaemonSpawnSpec};

use crate::daemon::launch_spec_args::encode_launch_spec_args;

use super::spec::{NetInterfaceSpec, NetLaunchSpec};

const READY_SIGNAL: u8 = b'R';
const NETD_RUNTIME_READ_PATHS: &[&str] = &[
    "/etc/resolv.conf",
    "/proc/self/cgroup",
    "/proc/stat",
    "/sys/devices/system/cpu/online",
];

pub struct NetDaemonAdapter;

#[derive(Debug)]
pub struct NetDaemonHandoff {
    host_fds: Vec<OwnedFd>,
    readiness_reader: Option<OwnedFd>,
    readiness_writer: Option<OwnedFd>,
}

impl NetDaemonHandoff {
    pub fn new(
        host_fds: Vec<OwnedFd>,
        readiness_reader: OwnedFd,
        readiness_writer: OwnedFd,
    ) -> Result<Self> {
        Ok(Self {
            host_fds,
            readiness_reader: Some(readiness_reader),
            readiness_writer: Some(readiness_writer),
        })
    }
}

#[derive(Debug)]
pub struct NetDaemonReadiness {
    ready_pipe_reader: OwnedFd,
}

impl DaemonReadiness for NetDaemonReadiness {
    fn wait_ready(self, timeout: Duration) -> Result<()> {
        let timeout_millis = timeout
            .as_millis()
            .min(i32::MAX as u128)
            .try_into()
            .expect("timeout clamped to i32::MAX");

        let mut poll_fd = libc::pollfd {
            fd: self.ready_pipe_reader.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };

        loop {
            // SAFETY: `poll_fd` points to valid memory for one `pollfd` entry.
            let rc = unsafe { libc::poll(&mut poll_fd as *mut libc::pollfd, 1, timeout_millis) };
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

        let mut ready_file = std::fs::File::from(self.ready_pipe_reader);
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
}

impl DaemonAdapter for NetDaemonAdapter {
    type Spec = NetLaunchSpec;
    type Handoff = NetDaemonHandoff;
    type Ready = NetDaemonReadiness;

    fn binary_info() -> DaemonBinaryInfo {
        DaemonBinaryInfo {
            daemon_name: "net",
            binary_name: "capsa-netd",
            env_override: "CAPSA_NETD_PATH",
        }
    }

    fn spawn_spec(
        spec: &Self::Spec,
        handoff: &mut Self::Handoff,
        binary_path: &Path,
    ) -> Result<DaemonSpawnSpec> {
        ensure!(
            spec.interfaces.len() == handoff.host_fds.len(),
            "net handoff host fd count ({}) must match interface count ({})",
            handoff.host_fds.len(),
            spec.interfaces.len()
        );

        let readiness_writer = handoff
            .readiness_writer
            .take()
            .context("missing net daemon readiness writer fd")?;

        let mut builder = capsa_sandbox::Sandbox::builder()
            .allow_network(true)
            .read_only_path(binary_path.to_path_buf());
        for runtime_read_path in NETD_RUNTIME_READ_PATHS {
            builder = builder.read_only_path(std::path::PathBuf::from(*runtime_read_path));
        }

        // Drain the host socketpairs from the handoff and hand them to
        // the sandbox builder. Each returned raw fd number is recorded
        // in the launch spec so netd can open it by number.
        let drained_host_fds: Vec<OwnedFd> = handoff.host_fds.drain(..).collect();
        let mut host_interfaces = Vec::with_capacity(drained_host_fds.len());
        for (host_fd, interface) in drained_host_fds.into_iter().zip(&spec.interfaces) {
            let host_raw = builder.inherit_fd(host_fd)?;
            host_interfaces.push(NetInterfaceSpec {
                host_fd: host_raw,
                mac: interface.mac,
                policy: interface.policy.clone(),
            });
        }

        let ready_raw = builder.inherit_fd(readiness_writer)?;

        // Rebuild the launch spec with kernel-assigned fd numbers so
        // netd can open each fd by the number it actually inherited.
        let runtime_spec = NetLaunchSpec {
            ready_fd: ready_raw,
            interfaces: host_interfaces,
            port_forwards: spec.port_forwards.clone(),
        };

        Ok(DaemonSpawnSpec {
            args: encode_launch_spec_args(&runtime_spec)?,
            sandbox: builder,
            stdin_null: true,
        })
    }

    fn readiness(_spec: &Self::Spec, handoff: &mut Self::Handoff) -> Result<Self::Ready> {
        let ready_pipe_reader = handoff
            .readiness_reader
            .take()
            .context("missing net daemon readiness reader fd")?;

        Ok(NetDaemonReadiness { ready_pipe_reader })
    }

    fn on_spawned(_spec: &Self::Spec, _handoff: &mut Self::Handoff) -> Result<()> {
        // `spawn_spec` already drained `host_fds` and took
        // `readiness_writer` into the fd remaps; nothing left to clean up.
        Ok(())
    }

    fn on_spawn_failed(_spec: &Self::Spec, _handoff: Self::Handoff) -> Result<()> {
        Ok(())
    }

    fn on_shutdown(_spec: &Self::Spec, _handoff: Self::Handoff) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd};
    use std::os::unix::net::UnixDatagram;

    use crate::daemon::traits::{DaemonAdapter, DaemonReadiness};

    use super::{NetDaemonAdapter, NetDaemonHandoff, NetDaemonReadiness};

    fn sample_host_fd() -> OwnedFd {
        let (left, _right) = UnixDatagram::pair().expect("socketpair should succeed");
        left.into()
    }

    fn pipe() -> (OwnedFd, OwnedFd) {
        let mut fds = [0; 2];
        // SAFETY: `fds` points to valid memory for two fds.
        let rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
        assert_eq!(rc, 0, "pipe creation should succeed");

        // SAFETY: pipe created valid read and write fds.
        let reader = unsafe { OwnedFd::from_raw_fd(fds[0]) };
        // SAFETY: pipe created valid read and write fds.
        let writer = unsafe { OwnedFd::from_raw_fd(fds[1]) };

        (reader, writer)
    }

    fn sample_spec() -> crate::daemon::net::spec::NetLaunchSpec {
        crate::daemon::net::spec::NetLaunchSpec {
            ready_fd: 30,
            interfaces: vec![crate::daemon::net::spec::NetInterfaceSpec {
                host_fd: 200,
                mac: [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee],
                policy: None,
            }],
            port_forwards: vec![],
        }
    }

    fn decode_runtime_spec(
        spawn_spec: &crate::daemon::traits::DaemonSpawnSpec,
    ) -> super::NetLaunchSpec {
        assert_eq!(
            spawn_spec.args[0], "--launch-spec-json",
            "first arg should be the JSON flag"
        );
        serde_json::from_str(&spawn_spec.args[1]).expect("spec args should be valid JSON")
    }

    #[test]
    fn netd_spawn_spec_encodes_runtime_fd_numbers_from_handoff() {
        let spec = sample_spec();
        let host = sample_host_fd();
        let host_raw = host.as_raw_fd();
        let (ready_r, ready_w) = pipe();
        let ready_raw = ready_w.as_raw_fd();
        let mut handoff =
            NetDaemonHandoff::new(vec![host], ready_r, ready_w).expect("handoff should build");

        let spawn_spec = NetDaemonAdapter::spawn_spec(
            &spec,
            &mut handoff,
            std::path::Path::new("/tmp/capsa-netd"),
        )
        .expect("spawn spec should build");

        assert!(spawn_spec.stdin_null);

        let runtime = decode_runtime_spec(&spawn_spec);
        // The runtime spec records the exact kernel-assigned fd
        // numbers the child will inherit, not arbitrary target slots.
        assert_eq!(runtime.ready_fd, ready_raw);
        assert_eq!(runtime.interfaces.len(), 1);
        assert_eq!(runtime.interfaces[0].host_fd, host_raw);
        assert_eq!(runtime.interfaces[0].mac, spec.interfaces[0].mac);
        // Sanity: the runtime spec passes its own validator.
        runtime
            .validate()
            .expect("runtime spec should pass NetLaunchSpec::validate");
    }

    #[test]
    fn readiness_waiter_accepts_exact_ready_byte() {
        let (ready_r, ready_w) = pipe();
        let mut writer = std::fs::File::from(ready_w);
        writer
            .write_all(b"R")
            .expect("write ready byte should succeed");
        drop(writer);

        NetDaemonReadiness {
            ready_pipe_reader: ready_r,
        }
        .wait_ready(std::time::Duration::from_secs(1))
        .expect("ready byte should be accepted");
    }

    #[test]
    fn readiness_waiter_rejects_wrong_byte() {
        let (ready_r, ready_w) = pipe();
        let mut writer = std::fs::File::from(ready_w);
        writer
            .write_all(b"X")
            .expect("write wrong byte should succeed");
        drop(writer);

        let err = NetDaemonReadiness {
            ready_pipe_reader: ready_r,
        }
        .wait_ready(std::time::Duration::from_secs(1))
        .expect_err("wrong byte must fail");

        assert!(err
            .to_string()
            .contains("invalid net daemon readiness byte"));
    }

    #[test]
    fn readiness_waiter_times_out_without_signal() {
        let (ready_r, ready_w) = pipe();
        let leaked_writer_fd = ready_w.into_raw_fd();

        let err = NetDaemonReadiness {
            ready_pipe_reader: ready_r,
        }
        .wait_ready(std::time::Duration::from_millis(10))
        .expect_err("missing signal should timeout");

        // SAFETY: close intentionally leaked writer fd used for timeout scenario.
        let _ = unsafe { libc::close(leaked_writer_fd) };

        assert!(err
            .to_string()
            .contains("timed out waiting for net daemon readiness signal"));
    }
}
