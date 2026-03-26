mod interface_plan;

use std::path::Path;

use anyhow::{ensure, Context, Result};

use std::os::fd::OwnedFd;

use capsa_net::{bridge_to_switch, GatewayStack, GatewayStackConfig, VirtualSwitch};

use tokio::runtime::{Builder, Runtime};
use tokio::task::JoinHandle;

use crate::{
    daemon::{resolve::resolve_daemon_binary, vmm::args::encode_launch_spec_args},
    ResolvedNetworkInterface, VmConfig, VmNetworkInterfaceConfig, VmmLaunchSpec,
};

use self::interface_plan::{
    build_interface_plan, resolved_interfaces_for_plan, vmm_fd_remaps_for_plan,
};

impl VmConfig {
    /// Start the VM in the sandboxed sidecar process.
    pub fn start(&self) -> Result<()> {
        self.validate().context("invalid VM configuration")?;

        let vmm_exe = resolve_vmm_binary()?;
        let spec = vm_sandbox_spec(self, &vmm_exe);

        let mut network_runtime = if self.interfaces.is_empty() {
            None
        } else {
            Some(
                NetworkRuntimeContext::start(&self.interfaces)
                    .context("failed to start network runtime")?,
            )
        };

        let launch_spec = VmmLaunchSpec {
            vm_config: self.clone(),
            resolved_interfaces: network_runtime
                .as_ref()
                .map(NetworkRuntimeContext::resolved_interfaces)
                .unwrap_or_default(),
        };

        let child_args = encode_launch_spec_args(&launch_spec)?;

        let child = {
            if let Some(runtime) = network_runtime.as_ref() {
                capsa_sandbox::spawn_sandboxed_with_fds(
                    &vmm_exe,
                    &child_args,
                    &spec,
                    runtime.fd_remaps(),
                )
            } else {
                capsa_sandbox::spawn_sandboxed(&vmm_exe, &child_args, &spec)
            }
        }
        .with_context(|| {
            format!(
                "failed to spawn sandboxed VMM process: {}",
                vmm_exe.display()
            )
        });

        let child = match child {
            Ok(child) => child,
            Err(err) => {
                if let Some(runtime) = network_runtime.take() {
                    runtime
                        .shutdown()
                        .context("failed to shutdown network runtime after spawn failure")?;
                }
                return Err(err);
            }
        };

        if let Some(runtime) = network_runtime.as_mut() {
            // Once spawn returns, child pre-exec fd remapping has already run.
            // We can drop the launcher-owned guest endpoints immediately.
            runtime.release_guest_fds_after_spawn();
        }

        let wait_result = child.wait().context("failed to wait on sandboxed child");

        if let Some(runtime) = network_runtime.take() {
            runtime
                .shutdown()
                .context("failed to shutdown network runtime")?;
        }

        let status = wait_result?;
        if status.success() {
            return Ok(());
        }

        anyhow::bail!("sandboxed VMM process exited with status {status}")
    }
}

#[derive(Debug)]
struct NetworkRuntimeContext {
    runtime: Runtime,
    network: NetworkRuntime,
}

impl NetworkRuntimeContext {
    fn start(interfaces: &[VmNetworkInterfaceConfig]) -> Result<Self> {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("failed to build tokio runtime for networking")?;
        let network = runtime
            .block_on(NetworkRuntime::start(interfaces))
            .context("failed to initialize network runtime")?;

        Ok(Self { runtime, network })
    }

    fn resolved_interfaces(&self) -> Vec<ResolvedNetworkInterface> {
        self.network.resolved_interfaces.clone()
    }

    fn fd_remaps(&self) -> &[capsa_sandbox::FdRemap] {
        &self.network.fd_remaps
    }

    fn release_guest_fds_after_spawn(&mut self) {
        self.network.guest_fds.clear();
    }

    fn shutdown(self) -> Result<()> {
        self.runtime.block_on(self.network.shutdown())
    }
}

#[derive(Debug)]
struct NetworkRuntime {
    resolved_interfaces: Vec<ResolvedNetworkInterface>,
    fd_remaps: Vec<capsa_sandbox::FdRemap>,
    guest_fds: Vec<OwnedFd>,
    bridge_tasks: Vec<JoinHandle<std::io::Result<()>>>,
    gateway_tasks: Vec<JoinHandle<std::io::Result<()>>>,
}

impl NetworkRuntime {
    async fn start(interfaces: &[VmNetworkInterfaceConfig]) -> Result<Self> {
        ensure!(
            !interfaces.is_empty(),
            "network runtime requires at least one interface"
        );

        let plan = build_interface_plan(interfaces)?;
        debug_assert_eq!(
            plan.interfaces.len(),
            plan.interfaces
                .iter()
                .map(|iface| iface.netd_host_target_fd)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            "planned netd host target fds must be unique"
        );
        let resolved_interfaces = resolved_interfaces_for_plan(&plan.interfaces);
        let fd_remaps = vmm_fd_remaps_for_plan(&plan.interfaces);
        let mut guest_fds = Vec::with_capacity(plan.interfaces.len());
        let mut bridge_tasks = Vec::with_capacity(plan.interfaces.len());
        let mut gateway_tasks = Vec::with_capacity(plan.interfaces.len());

        for planned_interface in plan.interfaces {
            let switch = VirtualSwitch::new();
            let vm_port = switch.create_port().await;
            let gateway_port = switch.create_port().await;

            let bridge_task =
                tokio::spawn(
                    async move { bridge_to_switch(planned_interface.host_fd, vm_port).await },
                );
            let gateway_config = gateway_config_for_policy(planned_interface.policy);
            let gateway = GatewayStack::new(gateway_port, gateway_config).await;
            let gateway_task = tokio::spawn(async move { gateway.run().await });

            guest_fds.push(planned_interface.guest_fd);
            bridge_tasks.push(bridge_task);
            gateway_tasks.push(gateway_task);
        }

        Ok(Self {
            resolved_interfaces,
            fd_remaps,
            guest_fds,
            bridge_tasks,
            gateway_tasks,
        })
    }

    async fn shutdown(self) -> Result<()> {
        for handle in &self.bridge_tasks {
            handle.abort();
        }
        for handle in &self.gateway_tasks {
            handle.abort();
        }

        for handle in self.bridge_tasks {
            let _ = handle.await;
        }
        for handle in self.gateway_tasks {
            let _ = handle.await;
        }

        Ok(())
    }
}

fn gateway_config_for_policy(policy: capsa_net::NetworkPolicy) -> GatewayStackConfig {
    GatewayStackConfig {
        policy: Some(policy),
        ..GatewayStackConfig::default()
    }
}

fn vm_sandbox_spec(config: &VmConfig, vmm_exe: &Path) -> capsa_sandbox::SandboxSpec {
    let mut spec = capsa_sandbox::SandboxSpec::new().allow_network(false);

    spec.read_only_paths.push(vmm_exe.to_path_buf());

    if let Some(root) = &config.root {
        spec.read_write_paths.push(root.clone());
    }

    if let Some(kernel) = &config.kernel {
        spec.read_only_paths.push(kernel.clone());
    }

    if let Some(initramfs) = &config.initramfs {
        spec.read_only_paths.push(initramfs.clone());
    }

    spec
}

fn resolve_vmm_binary() -> Result<std::path::PathBuf> {
    resolve_daemon_binary("capsa-vmm", "CAPSA_VMM_PATH")
}

#[cfg(test)]
mod tests {
    use super::{gateway_config_for_policy, vm_sandbox_spec};
    use crate::{VmConfig, VmNetworkInterfaceConfig};
    use capsa_net::{DomainPattern, NetworkPolicy};

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

    #[test]
    fn vm_sandbox_spec_disables_network_without_interfaces() {
        let config = sample_config();
        let spec = vm_sandbox_spec(&config, std::path::Path::new("/tmp/capsa-vmm"));

        assert!(!spec.allow_network);
    }

    #[test]
    fn start_rejects_multiple_interfaces_before_spawning_sidecar() {
        let mut config = sample_config();
        config.interfaces = vec![
            VmNetworkInterfaceConfig {
                mac: None,
                policy: None,
            },
            VmNetworkInterfaceConfig {
                mac: None,
                policy: None,
            },
        ];

        let err = config.start().expect_err("start should fail validation");
        let rendered = format!("{err:#}");
        assert!(rendered.contains("invalid VM configuration"));
        assert!(rendered.contains("multiple network interfaces are not supported yet"));
    }

    #[test]
    fn network_runtime_starts_and_stops_cleanly() {
        let interfaces = vec![VmNetworkInterfaceConfig {
            mac: None,
            policy: None,
        }];
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("runtime should build");

        let mut network = runtime
            .block_on(super::NetworkRuntime::start(&interfaces))
            .expect("network runtime should start");

        assert_eq!(network.resolved_interfaces.len(), 1);
        assert_eq!(network.fd_remaps.len(), 1);
        assert_eq!(network.guest_fds.len(), 1);

        network.guest_fds.clear();
        runtime
            .block_on(network.shutdown())
            .expect("network runtime should shut down");
    }

    #[test]
    fn network_runtime_preserves_explicit_mac() {
        let mac = [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee];
        let interfaces = vec![VmNetworkInterfaceConfig {
            mac: Some(mac),
            policy: None,
        }];
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("runtime should build");

        let mut network = runtime
            .block_on(super::NetworkRuntime::start(&interfaces))
            .expect("network runtime should start");

        assert_eq!(network.resolved_interfaces[0].mac, mac);

        network.guest_fds.clear();
        runtime
            .block_on(network.shutdown())
            .expect("network runtime should shut down");
    }

    #[test]
    fn network_runtime_rejects_all_zero_mac() {
        let interfaces = vec![VmNetworkInterfaceConfig {
            mac: Some([0; 6]),
            policy: None,
        }];
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("runtime should build");

        let err = runtime
            .block_on(super::NetworkRuntime::start(&interfaces))
            .expect_err("all-zero MAC should be rejected");

        assert!(err.to_string().contains("MAC address is all zeros"));
    }

    #[test]
    fn network_runtime_starts_bridge_task() {
        let interfaces = vec![VmNetworkInterfaceConfig {
            mac: None,
            policy: None,
        }];
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("runtime should build");

        let mut network = runtime
            .block_on(super::NetworkRuntime::start(&interfaces))
            .expect("network runtime should start");

        runtime.block_on(async {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        });

        assert_eq!(network.bridge_tasks.len(), 1);
        assert!(!network.bridge_tasks[0].is_finished());

        network.guest_fds.clear();
        runtime
            .block_on(network.shutdown())
            .expect("network runtime should shut down");
    }

    #[test]
    fn release_guest_fds_drops_launcher_copy_after_spawn() {
        let interfaces = vec![VmNetworkInterfaceConfig {
            mac: None,
            policy: None,
        }];
        let mut ctx = super::NetworkRuntimeContext::start(&interfaces)
            .expect("network runtime context should start");

        assert_eq!(ctx.network.guest_fds.len(), 1);
        ctx.release_guest_fds_after_spawn();
        assert!(ctx.network.guest_fds.is_empty());

        ctx.shutdown().expect("network runtime should shut down");
    }

    #[test]
    fn gateway_config_defaults_missing_interface_policy_to_deny_all() {
        let gateway_config = gateway_config_for_policy(NetworkPolicy::deny_all());

        assert_eq!(gateway_config.policy, Some(NetworkPolicy::deny_all()));
    }

    #[test]
    fn gateway_config_preserves_explicit_interface_policy() {
        let explicit_policy = NetworkPolicy::deny_all()
            .allow_domain(DomainPattern::parse("api.example.com").expect("pattern should parse"));

        let gateway_config = gateway_config_for_policy(explicit_policy.clone());

        assert_eq!(gateway_config.policy, Some(explicit_policy));
    }
}
