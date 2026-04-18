use std::future::pending;
use std::io;
use std::os::fd::{FromRawFd, OwnedFd};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use capsa_net::{
    bridge_to_switch, GatewayStack, GatewayStackConfig, NetworkPolicy, PortForwardRequest,
    VirtualSwitch,
};
use capsa_spec::{ControlResponse, NetLaunchSpec};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;

use crate::control::{run_control_loop, AttachInterface};

type TaskHandles = Arc<Mutex<Vec<JoinHandle<io::Result<()>>>>>;

const READY_SIGNAL: u8 = b'R';

pub async fn run(launch_spec: NetLaunchSpec, ready_fd: i32) -> Result<()> {
    let mut runtime = NetworkRuntime::start(launch_spec).await?;

    if let Err(err) = signal_readiness(ready_fd).context("failed to signal net daemon readiness") {
        runtime.abort_all().await;
        return Err(err);
    }

    runtime.wait_fail_fast().await
}

fn signal_readiness(ready_fd: i32) -> io::Result<()> {
    if ready_fd < 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid readiness fd: {ready_fd}"),
        ));
    }

    // SAFETY: `ready_fd` is provided by the launcher as a valid writable file descriptor,
    // and ownership is transferred to this function. `File::from_raw_fd` takes ownership
    // and closes the descriptor on drop.
    let mut ready_file = unsafe { std::fs::File::from_raw_fd(ready_fd) };
    use std::io::Write;
    ready_file.write_all(&[READY_SIGNAL])?;
    ready_file.flush()?;

    Ok(())
}

async fn run_port_forward_listener(
    listener: TcpListener,
    tx: mpsc::Sender<PortForwardRequest>,
    guest_ip: std::net::Ipv4Addr,
    guest_port: u16,
) -> io::Result<()> {
    loop {
        let (stream, _peer_addr) = listener.accept().await?;
        let request = PortForwardRequest {
            stream,
            guest_ip,
            guest_port,
        };

        if tx.send(request).await.is_err() {
            return Ok(());
        }
    }
}

struct NetworkRuntime {
    tasks: TaskHandles,
}

impl NetworkRuntime {
    async fn start(launch_spec: NetLaunchSpec) -> Result<Self> {
        let NetLaunchSpec {
            ready_fd: _,
            control_fd,
            interfaces,
            port_forwards,
        } = launch_spec;

        let tasks: TaskHandles = Arc::new(Mutex::new(Vec::with_capacity(
            interfaces.len() * 2 + port_forwards.len() + 1,
        )));

        for (index, interface) in interfaces.into_iter().enumerate() {
            // SAFETY: interface host fd values are validated by `NetLaunchSpec::validate` and
            // provided by the launcher after fd remapping. Ownership is transferred into netd.
            let host_fd = unsafe { OwnedFd::from_raw_fd(interface.host_fd) };
            let iface_forwards = if index == 0 {
                port_forwards.clone()
            } else {
                vec![]
            };
            setup_interface(
                host_fd,
                interface.mac,
                interface.policy,
                iface_forwards,
                &tasks,
            )
            .await
            .with_context(|| format!("failed to set up interface {index}"))?;
            tracing::debug!(interface = index, "netd interface runtime initialized");
        }

        if let Some(control_fd) = control_fd {
            // SAFETY: control_fd is validated by `NetLaunchSpec::validate` and inherited
            // from the launcher; ownership is transferred to the control loop.
            let control_fd = unsafe { OwnedFd::from_raw_fd(control_fd) };
            let tasks_for_handler = tasks.clone();
            let control_task: JoinHandle<io::Result<()>> = tokio::spawn(async move {
                let result = run_control_loop(control_fd, move |iface: AttachInterface| {
                    let tasks = tasks_for_handler.clone();
                    async move {
                        match setup_interface(
                            iface.host_fd,
                            iface.mac,
                            None,
                            iface.port_forwards,
                            &tasks,
                        )
                        .await
                        {
                            Ok(()) => ControlResponse::Ok,
                            Err(err) => ControlResponse::Error {
                                message: err.to_string(),
                            },
                        }
                    }
                })
                .await;
                result.map_err(io::Error::other)
            });
            tasks.lock().await.push(control_task);
            tracing::debug!("netd control loop initialized");
        }

        Ok(Self { tasks })
    }

    async fn wait_fail_fast(&mut self) -> Result<()> {
        loop {
            let (is_empty, completed_idx) = {
                let guard = self.tasks.lock().await;
                let empty = guard.is_empty();
                let idx = if empty {
                    None
                } else {
                    (0..guard.len()).find(|&i| guard[i].is_finished())
                };
                (empty, idx)
            };

            if is_empty {
                pending::<()>().await;
                unreachable!("pending future should never complete");
            }

            if let Some(idx) = completed_idx {
                let completed = self.tasks.lock().await.swap_remove(idx);
                let result = match completed.await {
                    Ok(Ok(())) => Err(anyhow!(
                        "critical network task exited unexpectedly without error"
                    )),
                    Ok(Err(err)) => Err(anyhow!("critical network task failed: {err}")),
                    Err(join_err) => Err(anyhow!("critical network task panicked: {join_err}")),
                };

                self.abort_all().await;
                return result;
            }

            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    async fn abort_all(&mut self) {
        let handles: Vec<JoinHandle<io::Result<()>>> =
            std::mem::take(&mut *self.tasks.lock().await);
        for handle in &handles {
            handle.abort();
        }
        for handle in handles {
            let _ = handle.await;
        }
    }
}

async fn setup_interface(
    host_fd: OwnedFd,
    mac: [u8; 6],
    policy: Option<NetworkPolicy>,
    port_forwards: Vec<(u16, u16)>,
    tasks: &TaskHandles,
) -> Result<()> {
    let switch = VirtualSwitch::new();
    let vm_port = switch.create_port().await;
    let gateway_port = switch.create_port().await;

    let gateway_config = GatewayStackConfig {
        policy: Some(policy.unwrap_or_else(NetworkPolicy::deny_all)),
        port_forwards: port_forwards.clone(),
        ..GatewayStackConfig::default()
    };
    let guest_ip = gateway_config.dhcp_range_start;
    let _ = mac; // reserved for future MAC-aware routing in shared-subnet mode

    let (port_forward_tx, port_forward_rx) = mpsc::channel(64);

    let bridge_task = tokio::spawn(async move { bridge_to_switch(host_fd, vm_port).await });
    let gateway = GatewayStack::new(gateway_port, gateway_config)
        .await
        .with_port_forward_rx(port_forward_rx);
    let gateway_task = tokio::spawn(async move { gateway.run().await });

    {
        let mut guard = tasks.lock().await;
        guard.push(bridge_task);
        guard.push(gateway_task);
    }

    for (host_port, guest_port) in port_forwards {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, host_port))
            .await
            .with_context(|| {
                format!("failed to bind TCP port forward listener on host port {host_port}")
            })?;
        let tx = port_forward_tx.clone();
        let listener_task = tokio::spawn(async move {
            run_port_forward_listener(listener, tx, guest_ip, guest_port).await
        });
        tasks.lock().await.push(listener_task);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run;
    use capsa_net::NetworkPolicy;
    use capsa_spec::{NetInterfaceSpec, NetLaunchSpec};
    use std::io::Read;
    use std::os::fd::{FromRawFd, IntoRawFd};
    use std::os::unix::net::UnixDatagram;

    fn sample_interface(host_fd: i32, policy: Option<NetworkPolicy>) -> NetInterfaceSpec {
        NetInterfaceSpec {
            host_fd,
            mac: [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee],
            policy,
        }
    }

    fn pipe() -> (std::fs::File, i32) {
        let mut fds = [0; 2];
        // SAFETY: `fds` points to valid memory for two integers.
        let rc = unsafe { libc::pipe(fds.as_mut_ptr()) };
        assert_eq!(rc, 0, "pipe creation must succeed");

        // SAFETY: `pipe` filled `fds[0]` with a valid read descriptor.
        let reader = unsafe { std::fs::File::from_raw_fd(fds[0]) };
        (reader, fds[1])
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn task_failure_causes_non_zero_daemon_outcome() {
        let mut host_pipe_fds = [0; 2];
        // SAFETY: `host_pipe_fds` points to valid memory for two integers.
        let rc = unsafe { libc::pipe(host_pipe_fds.as_mut_ptr()) };
        assert_eq!(
            rc, 0,
            "pipe creation for invalid host endpoint should succeed"
        );
        let host_fd = host_pipe_fds[0];
        // SAFETY: close the write end; runtime takes ownership of `host_fd` only.
        let close_rc = unsafe { libc::close(host_pipe_fds[1]) };
        assert_eq!(close_rc, 0, "closing write end should succeed");

        let (mut reader, writer_fd) = pipe();

        let launch_spec = NetLaunchSpec {
            ready_fd: writer_fd,
            control_fd: None,
            interfaces: vec![sample_interface(host_fd, None)],
            port_forwards: vec![],
        };

        let err = run(launch_spec, writer_fd)
            .await
            .expect_err("bridge task completion should fail-fast netd");

        let mut ready = [0u8; 1];
        reader
            .read_exact(&mut ready)
            .expect("readiness byte should be emitted before task failure");
        assert_eq!(ready, [super::READY_SIGNAL]);

        assert!(err.to_string().contains("critical network task"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn control_fd_peer_close_fails_daemon_fast() {
        use nix::sys::socket::{socketpair, AddressFamily, SockFlag, SockType};
        use std::os::fd::IntoRawFd;

        let (server, client) = socketpair(
            AddressFamily::Unix,
            SockType::SeqPacket,
            None,
            SockFlag::SOCK_CLOEXEC,
        )
        .expect("seqpacket pair");
        let server_fd = server.into_raw_fd();

        let (_reader, writer_fd) = pipe();
        let launch_spec = NetLaunchSpec {
            ready_fd: writer_fd,
            control_fd: Some(server_fd),
            interfaces: vec![],
            port_forwards: vec![],
        };

        // Close the peer so the control task exits cleanly once
        // readiness is signalled; wait_fail_fast should flag the
        // unexpected exit.
        drop(client);

        let err = run(launch_spec, writer_fd)
            .await
            .expect_err("control task exiting should fail-fast netd");
        assert!(
            err.to_string().contains("critical network task"),
            "unexpected: {err}"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn readiness_emitted_once_after_startup_path() {
        let (host, _guest) = UnixDatagram::pair().expect("socketpair should be created");
        let host_fd = host.into_raw_fd();
        let (mut reader, writer_fd) = pipe();
        let launch_spec = NetLaunchSpec {
            ready_fd: writer_fd,
            control_fd: None,
            interfaces: vec![sample_interface(host_fd, None)],
            port_forwards: vec![],
        };

        let daemon = tokio::spawn(async move { run(launch_spec, writer_fd).await });

        let (ready, rest) = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            tokio::task::spawn_blocking(move || {
                let mut ready = [0u8; 1];
                reader
                    .read_exact(&mut ready)
                    .expect("readiness byte should be readable");

                let mut rest = Vec::new();
                reader
                    .read_to_end(&mut rest)
                    .expect("pipe should close after readiness write");

                (ready, rest)
            }),
        )
        .await
        .expect("readiness read should complete")
        .expect("readiness read task should not panic");

        assert_eq!(ready, [super::READY_SIGNAL]);
        assert!(rest.is_empty(), "readiness should be emitted exactly once");

        daemon.abort();
        let _ = daemon.await;
    }
}
