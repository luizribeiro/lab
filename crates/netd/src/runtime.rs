use std::future::pending;
use std::io;
use std::os::fd::{FromRawFd, OwnedFd};

use anyhow::{anyhow, Context, Result};
use capsa_core::daemon::net::spec::{NetInterfaceSpec, NetLaunchSpec};
use capsa_net::{bridge_to_switch, GatewayStack, GatewayStackConfig, NetworkPolicy, VirtualSwitch};
use tokio::task::JoinHandle;

const READY_SIGNAL: u8 = b'R';

pub async fn run(launch_spec: NetLaunchSpec, ready_fd: i32) -> Result<()> {
    let mut runtime = NetworkRuntime::start(launch_spec).await?;

    if let Err(err) = signal_readiness(ready_fd).context("failed to signal net daemon readiness") {
        runtime.abort_all().await;
        return Err(err);
    }

    runtime.wait_fail_fast().await
}

fn gateway_config_for_interface(interface: &NetInterfaceSpec) -> GatewayStackConfig {
    GatewayStackConfig {
        policy: Some(
            interface
                .policy
                .clone()
                .unwrap_or_else(NetworkPolicy::deny_all),
        ),
        ..GatewayStackConfig::default()
    }
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
    tasks: Vec<JoinHandle<io::Result<()>>>,
}

impl NetworkRuntime {
    async fn start(launch_spec: NetLaunchSpec) -> Result<Self> {
        let mut tasks = Vec::with_capacity(launch_spec.interfaces.len() * 2);

        for (index, interface) in launch_spec.interfaces.into_iter().enumerate() {
            // SAFETY: interface host fd values are validated by `NetLaunchSpec::validate` and
            // provided by the launcher after fd remapping. Ownership is transferred into netd.
            let host_fd = unsafe { OwnedFd::from_raw_fd(interface.host_fd) };

            let switch = VirtualSwitch::new();
            let vm_port = switch.create_port().await;
            let gateway_port = switch.create_port().await;

            let bridge_task = tokio::spawn(async move { bridge_to_switch(host_fd, vm_port).await });
            let gateway =
                GatewayStack::new(gateway_port, gateway_config_for_interface(&interface)).await;
            let gateway_task = tokio::spawn(async move { gateway.run().await });

            tasks.push(bridge_task);
            tasks.push(gateway_task);

            tracing::debug!(interface = index, "netd interface runtime initialized");
        }

        Ok(Self { tasks })
    }

    async fn wait_fail_fast(&mut self) -> Result<()> {
        if self.tasks.is_empty() {
            pending::<()>().await;
            unreachable!("pending future should never complete");
        }

        loop {
            for index in 0..self.tasks.len() {
                if self.tasks[index].is_finished() {
                    let completed = self.tasks.swap_remove(index);
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
            }

            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }

    async fn abort_all(&mut self) {
        for task in &self.tasks {
            task.abort();
        }

        while let Some(task) = self.tasks.pop() {
            let _ = task.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::run;
    use capsa_core::daemon::net::spec::{NetInterfaceSpec, NetLaunchSpec};
    use capsa_net::{DomainPattern, NetworkPolicy};
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

    #[test]
    fn explicit_policy_reaches_gateway_stack_config() {
        let explicit_policy = NetworkPolicy::deny_all()
            .allow_domain(DomainPattern::parse("api.example.com").expect("pattern should parse"));
        let interface = sample_interface(200, Some(explicit_policy.clone()));

        let config = super::gateway_config_for_interface(&interface);

        assert_eq!(config.policy, Some(explicit_policy));
    }

    #[test]
    fn omitted_policy_falls_back_to_deny_all() {
        let interface = sample_interface(200, None);

        let config = super::gateway_config_for_interface(&interface);

        assert_eq!(config.policy, Some(NetworkPolicy::deny_all()));
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
            interfaces: vec![sample_interface(host_fd, None)],
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
    async fn readiness_emitted_once_after_startup_path() {
        let (host, _guest) = UnixDatagram::pair().expect("socketpair should be created");
        let host_fd = host.into_raw_fd();
        let launch_spec = NetLaunchSpec {
            interfaces: vec![sample_interface(host_fd, None)],
        };

        let (mut reader, writer_fd) = pipe();

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
