use std::future::pending;
use std::io;
use std::os::fd::{FromRawFd, OwnedFd};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use capsa_net::{bridge_to_switch, GatewayStack, GatewayStackConfig, NetworkPolicy, VirtualSwitch};
use capsa_spec::{ControlResponse, NetLaunchSpec};
use tokio::sync::Mutex;
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
        let gateway = GatewayStack::new(gateway_port, gateway_config).await;
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
                async move {
                    match attach_interface_port(iface, switch.as_ref(), &tasks).await {
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

async fn attach_interface_port(
    iface: AttachInterface,
    switch: &VirtualSwitch,
    tasks: &TaskHandles,
) -> Result<()> {
    let AttachInterface {
        mac: _,
        port_forwards,
        host_fd,
    } = iface;

    // TODO(shared-subnet): per-interface port forwards require
    // knowing the guest's DHCP IP. Plumbing that through the shared
    // gateway is a follow-up; for now log and drop them.
    if !port_forwards.is_empty() {
        tracing::warn!(
            count = port_forwards.len(),
            "AddInterface port_forwards are not yet supported under the shared gateway; ignoring"
        );
    }

    let vm_port = switch.create_port().await;
    let bridge_task: JoinHandle<io::Result<()>> =
        tokio::spawn(async move { bridge_to_switch(host_fd, vm_port).await });
    tasks.lock().await.push(bridge_task);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run;
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
}
