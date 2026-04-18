use std::future::pending;
use std::io;
use std::net::Ipv4Addr;
use std::os::fd::{FromRawFd, OwnedFd};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use capsa_net::{
    bridge_to_switch, GatewayStack, GatewayStackConfig, LeasePreallocator, NetworkPolicy,
    PortForwardRequest, UdpPortForwardRequest, VirtualSwitch,
};
use capsa_spec::{ControlResponse, NetLaunchSpec};
use tokio::net::{TcpListener, UdpSocket};
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

struct NetworkRuntime {
    tasks: TaskHandles,
}

impl NetworkRuntime {
    async fn start(launch_spec: NetLaunchSpec) -> Result<Self> {
        let NetLaunchSpec {
            ready_fd: _,
            control_fd,
            policy,
        } = launch_spec;

        let tasks: TaskHandles = Arc::new(Mutex::new(Vec::new()));

        let Some(control_fd) = control_fd else {
            return Ok(Self { tasks });
        };

        // Build the single shared switch + gateway for this daemon.
        // Every attached interface becomes another port on this
        // switch; every outbound flow hits this one gateway stack.
        let switch = Arc::new(VirtualSwitch::new());
        let gateway_port = switch.create_port().await;
        let gateway_config = GatewayStackConfig {
            policy: Some(policy.unwrap_or_else(NetworkPolicy::deny_all)),
            ..GatewayStackConfig::default()
        };
        let (port_forward_tx, port_forward_rx) =
            mpsc::channel::<PortForwardRequest>(PORT_FORWARD_CHANNEL_CAPACITY);
        let (udp_port_forward_tx, udp_port_forward_rx) =
            mpsc::channel::<UdpPortForwardRequest>(UDP_PORT_FORWARD_CHANNEL_CAPACITY);
        let gateway = GatewayStack::new(gateway_port, gateway_config)
            .await
            .with_port_forward_rx(port_forward_rx)
            .with_udp_port_forward_rx(udp_port_forward_rx);
        let preallocator = gateway.lease_preallocator();
        let gateway_task: JoinHandle<io::Result<()>> =
            tokio::spawn(async move { gateway.run().await });
        tasks.lock().await.push(gateway_task);

        // SAFETY: control_fd is validated by `NetLaunchSpec::validate`
        // and inherited from the launcher; ownership transferred in.
        let control_fd = unsafe { OwnedFd::from_raw_fd(control_fd) };
        let tasks_for_handler = tasks.clone();
        let switch_for_handler = switch.clone();
        let control_task: JoinHandle<io::Result<()>> = tokio::spawn(async move {
            let result = run_control_loop(control_fd, move |iface: AttachInterface| {
                let tasks = tasks_for_handler.clone();
                let switch = switch_for_handler.clone();
                let preallocator = preallocator.clone();
                let port_forward_tx = port_forward_tx.clone();
                let udp_port_forward_tx = udp_port_forward_tx.clone();
                async move {
                    match attach_interface_port(
                        iface,
                        switch.as_ref(),
                        &tasks,
                        &preallocator,
                        &port_forward_tx,
                        &udp_port_forward_tx,
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
        tracing::debug!("netd runtime initialized with shared gateway");

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

const PORT_FORWARD_CHANNEL_CAPACITY: usize = 64;
const UDP_PORT_FORWARD_CHANNEL_CAPACITY: usize = 64;
const UDP_RECV_BUF: usize = 4096;

async fn attach_interface_port(
    iface: AttachInterface,
    switch: &VirtualSwitch,
    tasks: &TaskHandles,
    preallocator: &LeasePreallocator,
    port_forward_tx: &mpsc::Sender<PortForwardRequest>,
    udp_port_forward_tx: &mpsc::Sender<UdpPortForwardRequest>,
) -> Result<()> {
    let AttachInterface {
        mac,
        port_forwards,
        udp_forwards,
        host_fd,
    } = iface;

    // Bind TCP and UDP listeners first: if any host port is
    // unavailable we want the AddInterface to fail before standing
    // up the bridge so the caller sees a clean error instead of a
    // half-wired interface.
    let needs_guest_ip = !port_forwards.is_empty() || !udp_forwards.is_empty();
    let guest_ip = if needs_guest_ip {
        Some(
            preallocator
                .preallocate(mac)
                .await
                .with_context(|| format!("failed to preallocate DHCP lease for MAC {mac:02x?}"))?,
        )
    } else {
        None
    };

    let mut tcp_listeners = Vec::with_capacity(port_forwards.len());
    for (host_port, guest_port) in port_forwards {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, host_port))
            .await
            .with_context(|| {
                format!("failed to bind TCP port forward listener on host port {host_port}")
            })?;
        tcp_listeners.push((listener, guest_ip.unwrap(), guest_port));
    }

    let mut udp_sockets = Vec::with_capacity(udp_forwards.len());
    for (host_port, guest_port) in udp_forwards {
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, host_port))
            .await
            .with_context(|| {
                format!("failed to bind UDP port forward listener on host port {host_port}")
            })?;
        udp_sockets.push((Arc::new(socket), guest_ip.unwrap(), guest_port));
    }

    let vm_port = switch.create_port().await;
    let bridge_task: JoinHandle<io::Result<()>> =
        tokio::spawn(async move { bridge_to_switch(host_fd, vm_port).await });

    let mut task_vec = tasks.lock().await;
    task_vec.push(bridge_task);
    for (listener, guest_ip, guest_port) in tcp_listeners {
        let tx = port_forward_tx.clone();
        let listener_task: JoinHandle<io::Result<()>> = tokio::spawn(async move {
            run_port_forward_listener(listener, tx, guest_ip, guest_port).await
        });
        task_vec.push(listener_task);
    }
    for (socket, guest_ip, guest_port) in udp_sockets {
        let tx = udp_port_forward_tx.clone();
        let socket_task: JoinHandle<io::Result<()>> = tokio::spawn(async move {
            run_udp_port_forward_listener(socket, tx, guest_ip, guest_port).await
        });
        task_vec.push(socket_task);
    }
    Ok(())
}

async fn run_port_forward_listener(
    listener: TcpListener,
    tx: mpsc::Sender<PortForwardRequest>,
    guest_ip: Ipv4Addr,
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

async fn run_udp_port_forward_listener(
    socket: Arc<UdpSocket>,
    tx: mpsc::Sender<UdpPortForwardRequest>,
    guest_ip: Ipv4Addr,
    guest_port: u16,
) -> io::Result<()> {
    let mut buf = vec![0u8; UDP_RECV_BUF];
    loop {
        let (len, peer) = socket.recv_from(&mut buf).await?;
        let host_src = match peer {
            std::net::SocketAddr::V4(v4) => v4,
            std::net::SocketAddr::V6(_) => {
                tracing::debug!(
                    host_port = socket.local_addr().ok().map(|a| a.port()),
                    "UDP forward: ignoring IPv6 datagram"
                );
                continue;
            }
        };
        let request = UdpPortForwardRequest {
            data: buf[..len].to_vec(),
            host_src,
            host_socket: socket.clone(),
            guest_ip,
            guest_port,
        };

        if tx.send(request).await.is_err() {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{attach_interface_port, run};
    use crate::control::AttachInterface;
    use capsa_net::{GatewayStack, GatewayStackConfig, VirtualSwitch};
    use capsa_spec::NetLaunchSpec;
    use std::io::Read;
    use std::os::fd::FromRawFd;

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
            policy: None,
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
    async fn readiness_emitted_exactly_once() {
        let (mut reader, writer_fd) = pipe();
        let launch_spec = NetLaunchSpec {
            ready_fd: writer_fd,
            control_fd: None,
            policy: None,
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

    async fn setup_preallocator_and_channels() -> (
        capsa_net::LeasePreallocator,
        tokio::sync::mpsc::Sender<capsa_net::PortForwardRequest>,
        tokio::sync::mpsc::Sender<capsa_net::UdpPortForwardRequest>,
    ) {
        let switch = VirtualSwitch::new();
        let gateway_port = switch.create_port().await;
        let (pf_tx, pf_rx) = tokio::sync::mpsc::channel(16);
        let (udp_tx, udp_rx) = tokio::sync::mpsc::channel(16);
        let gateway = GatewayStack::new(gateway_port, GatewayStackConfig::default())
            .await
            .with_port_forward_rx(pf_rx)
            .with_udp_port_forward_rx(udp_rx);
        let preallocator = gateway.lease_preallocator();
        tokio::spawn(async move { gateway.run().await });
        (preallocator, pf_tx, udp_tx)
    }

    fn dummy_host_fd() -> std::os::fd::OwnedFd {
        use nix::sys::socket::{socketpair, AddressFamily, SockFlag, SockType};
        use std::os::fd::{AsFd, OwnedFd};
        let (a, _b) = socketpair(
            AddressFamily::Unix,
            SockType::Datagram,
            None,
            SockFlag::SOCK_CLOEXEC,
        )
        .expect("socketpair");
        OwnedFd::from(a.as_fd().try_clone_to_owned().expect("clone"))
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn attach_interface_port_binds_host_listener() {
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let (preallocator, pf_tx, udp_tx) = setup_preallocator_and_channels().await;
        let switch = Arc::new(VirtualSwitch::new());
        let tasks = Arc::new(Mutex::new(Vec::new()));

        let probe = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("probe bind");
        let host_port = probe.local_addr().expect("local_addr").port();
        drop(probe);

        let iface = AttachInterface {
            mac: [0x52, 0x54, 0x00, 0x00, 0x00, 0x10],
            port_forwards: vec![(host_port, 80)],
            udp_forwards: vec![],
            host_fd: dummy_host_fd(),
        };

        attach_interface_port(
            iface,
            switch.as_ref(),
            &tasks,
            &preallocator,
            &pf_tx,
            &udp_tx,
        )
        .await
        .expect("attach should succeed");

        // Listener + bridge tasks should have been spawned.
        assert_eq!(tasks.lock().await.len(), 2);

        // The host port should now be bound by our listener.
        let rebind = tokio::net::TcpListener::bind(("127.0.0.1", host_port)).await;
        assert!(
            rebind.is_err(),
            "host_port {host_port} should already be bound by the forward listener"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn attach_interface_port_fails_when_host_port_busy() {
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let (preallocator, pf_tx, udp_tx) = setup_preallocator_and_channels().await;
        let switch = Arc::new(VirtualSwitch::new());
        let tasks = Arc::new(Mutex::new(Vec::new()));

        let blocker = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("blocker bind");
        let host_port = blocker.local_addr().expect("local_addr").port();

        let iface = AttachInterface {
            mac: [0x52, 0x54, 0x00, 0x00, 0x00, 0x11],
            port_forwards: vec![(host_port, 80)],
            udp_forwards: vec![],
            host_fd: dummy_host_fd(),
        };

        let err = attach_interface_port(
            iface,
            switch.as_ref(),
            &tasks,
            &preallocator,
            &pf_tx,
            &udp_tx,
        )
        .await
        .expect_err("bind on busy port should fail");
        assert!(
            err.to_string().contains("failed to bind"),
            "unexpected: {err}"
        );
        assert!(
            tasks.lock().await.is_empty(),
            "no bridge task should be spawned when bind fails"
        );

        drop(blocker);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn attach_interface_port_binds_udp_host_listener() {
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let (preallocator, pf_tx, udp_tx) = setup_preallocator_and_channels().await;
        let switch = Arc::new(VirtualSwitch::new());
        let tasks = Arc::new(Mutex::new(Vec::new()));

        let probe = tokio::net::UdpSocket::bind("127.0.0.1:0")
            .await
            .expect("probe bind");
        let host_port = probe.local_addr().expect("local_addr").port();
        drop(probe);

        let iface = AttachInterface {
            mac: [0x52, 0x54, 0x00, 0x00, 0x00, 0x20],
            port_forwards: vec![],
            udp_forwards: vec![(host_port, 53)],
            host_fd: dummy_host_fd(),
        };

        attach_interface_port(
            iface,
            switch.as_ref(),
            &tasks,
            &preallocator,
            &pf_tx,
            &udp_tx,
        )
        .await
        .expect("attach should succeed");

        // Listener + bridge tasks should have been spawned.
        assert_eq!(tasks.lock().await.len(), 2);

        // Host port should now be bound by the UDP listener.
        let rebind = tokio::net::UdpSocket::bind(("127.0.0.1", host_port)).await;
        assert!(
            rebind.is_err(),
            "host_port {host_port} should already be bound"
        );
    }
}
