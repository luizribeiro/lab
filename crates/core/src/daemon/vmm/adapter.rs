use std::os::fd::OwnedFd;
use std::path::Path;

use anyhow::{ensure, Result};

use crate::daemon::traits::{DaemonAdapter, DaemonBinaryInfo, DaemonSpawnSpec, NoReadiness};

use crate::daemon::launch_spec_args::encode_launch_spec_args;
use crate::ResolvedNetworkInterface;

use super::spec::VmmLaunchSpec;

pub struct VmmDaemonAdapter;

#[derive(Debug)]
pub struct VmmDaemonHandoff {
    guest_fds: Vec<OwnedFd>,
}

impl VmmDaemonHandoff {
    pub fn new(guest_fds: Vec<OwnedFd>) -> Result<Self> {
        Ok(Self { guest_fds })
    }
}

impl DaemonAdapter for VmmDaemonAdapter {
    type Spec = VmmLaunchSpec;
    type Handoff = VmmDaemonHandoff;
    type Ready = NoReadiness;

    fn binary_info() -> DaemonBinaryInfo {
        DaemonBinaryInfo {
            daemon_name: "vmm",
            binary_name: "capsa-vmm",
            env_override: "CAPSA_VMM_PATH",
        }
    }

    fn spawn_spec(
        spec: &Self::Spec,
        handoff: &mut Self::Handoff,
        binary_path: &Path,
    ) -> Result<DaemonSpawnSpec> {
        ensure!(
            spec.resolved_interfaces.len() == handoff.guest_fds.len(),
            "vmm handoff guest fd count ({}) must match resolved interface count ({})",
            handoff.guest_fds.len(),
            spec.resolved_interfaces.len()
        );

        let mut builder = vmm_sandbox_builder(&spec.vm_config, binary_path);

        // Drain the guest-side socketpair fds from the handoff and
        // hand them to the sandbox builder. Each returned raw fd
        // number is recorded in the VMM launch spec so libkrun can
        // attach to it by number.
        let drained_guest_fds: Vec<OwnedFd> = handoff.guest_fds.drain(..).collect();
        let mut resolved_interfaces = Vec::with_capacity(drained_guest_fds.len());
        for (guest_fd, interface) in drained_guest_fds.into_iter().zip(&spec.resolved_interfaces) {
            let guest_raw = builder.inherit_fd(guest_fd)?;
            resolved_interfaces.push(ResolvedNetworkInterface {
                mac: interface.mac,
                guest_fd: guest_raw,
            });
        }

        let runtime_spec = VmmLaunchSpec {
            vm_config: spec.vm_config.clone(),
            resolved_interfaces,
        };

        Ok(DaemonSpawnSpec {
            args: encode_launch_spec_args(&runtime_spec)?,
            sandbox: builder,
            stdin_null: false,
        })
    }

    fn readiness(_spec: &Self::Spec, _handoff: &mut Self::Handoff) -> Result<Self::Ready> {
        Ok(NoReadiness)
    }

    fn on_spawned(_spec: &Self::Spec, _handoff: &mut Self::Handoff) -> Result<()> {
        // `spawn_spec` already drained `guest_fds` into the fd remaps.
        Ok(())
    }

    fn on_spawn_failed(_spec: &Self::Spec, _handoff: Self::Handoff) -> Result<()> {
        Ok(())
    }

    fn on_shutdown(_spec: &Self::Spec, _handoff: Self::Handoff) -> Result<()> {
        Ok(())
    }
}

fn vmm_sandbox_builder(config: &crate::VmConfig, vmm_exe: &Path) -> capsa_sandbox::SandboxBuilder {
    let mut builder = capsa_sandbox::Sandbox::builder()
        .allow_network(false)
        .allow_kvm(true)
        .allow_interactive_tty(true)
        .read_only_path(vmm_exe.to_path_buf());

    if let Some(root) = &config.root {
        builder = builder.read_write_path(root.clone());
    }

    if let Some(kernel) = &config.kernel {
        builder = builder.read_only_path(kernel.clone());
    }

    if let Some(initramfs) = &config.initramfs {
        builder = builder.read_only_path(initramfs.clone());
    }

    builder
}

#[cfg(test)]
mod tests {
    use std::os::fd::{AsRawFd, OwnedFd};
    use std::os::unix::net::UnixDatagram;

    use crate::daemon::traits::DaemonAdapter;
    use crate::{ResolvedNetworkInterface, VmConfig, VmmLaunchSpec};

    use super::{VmmDaemonAdapter, VmmDaemonHandoff};

    fn sample_vm_config() -> VmConfig {
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

    fn sample_spec() -> VmmLaunchSpec {
        VmmLaunchSpec {
            vm_config: sample_vm_config(),
            resolved_interfaces: vec![ResolvedNetworkInterface {
                mac: [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee],
                guest_fd: 0, // placeholder; the adapter overwrites
            }],
        }
    }

    fn sample_guest_fd() -> OwnedFd {
        let (_left, right) = UnixDatagram::pair().expect("socketpair should succeed");
        right.into()
    }

    fn decode_runtime_spec(spawn_spec: &crate::daemon::traits::DaemonSpawnSpec) -> VmmLaunchSpec {
        assert_eq!(
            spawn_spec.args[0], "--launch-spec-json",
            "first arg should be the JSON flag"
        );
        serde_json::from_str(&spawn_spec.args[1]).expect("spec args should be valid JSON")
    }

    #[test]
    fn vmm_spawn_spec_encodes_runtime_guest_fd_from_handoff() {
        let spec = sample_spec();
        let source = sample_guest_fd();
        let source_raw = source.as_raw_fd();
        let mut handoff = VmmDaemonHandoff::new(vec![source]).expect("handoff should build");

        let spawn_spec = VmmDaemonAdapter::spawn_spec(
            &spec,
            &mut handoff,
            std::path::Path::new("/tmp/capsa-vmm"),
        )
        .expect("spawn spec should build");

        assert!(!spawn_spec.stdin_null);

        let runtime_spec = decode_runtime_spec(&spawn_spec);
        assert_eq!(runtime_spec.resolved_interfaces.len(), 1);
        assert_eq!(
            runtime_spec.resolved_interfaces[0].mac,
            [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]
        );
        // The runtime spec carries the same raw fd number that the
        // OwnedFd had in the parent, because the sandbox builder
        // inherits fds at their current numbers rather than remapping.
        assert_eq!(runtime_spec.resolved_interfaces[0].guest_fd, source_raw);
        assert!(
            runtime_spec.resolved_interfaces[0].guest_fd >= 3,
            "kernel-assigned fd should be >= 3"
        );
    }

    #[test]
    fn vmm_spawn_spec_preserves_vm_config() {
        let spec = sample_spec();
        let mut handoff =
            VmmDaemonHandoff::new(vec![sample_guest_fd()]).expect("handoff should build");

        let spawn_spec = VmmDaemonAdapter::spawn_spec(
            &spec,
            &mut handoff,
            std::path::Path::new("/tmp/capsa-vmm"),
        )
        .expect("spawn spec should build");

        let runtime_spec = decode_runtime_spec(&spawn_spec);
        assert_eq!(runtime_spec.vm_config.kernel, spec.vm_config.kernel);
        assert_eq!(runtime_spec.vm_config.root, spec.vm_config.root);
        assert_eq!(runtime_spec.vm_config.vcpus, spec.vm_config.vcpus);
    }
}
