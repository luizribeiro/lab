use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::path::Path;

use anyhow::{ensure, Context, Result};

use crate::daemon::traits::{DaemonAdapter, DaemonBinaryInfo, DaemonSpawnSpec, NoReadiness};

use crate::daemon::launch_spec_args::encode_launch_spec_args;

use super::spec::VmmLaunchSpec;

const MIN_REMAP_SOURCE_FD: i32 = 1000;

pub struct VmmDaemonAdapter;

#[derive(Debug)]
pub struct VmmDaemonHandoff {
    guest_fds: Vec<OwnedFd>,
}

impl VmmDaemonHandoff {
    pub fn new(guest_fds: Vec<OwnedFd>) -> Result<Self> {
        let mut normalized = Vec::with_capacity(guest_fds.len());
        for guest_fd in guest_fds {
            normalized.push(duplicate_fd_for_remap(&guest_fd)?);
        }

        Ok(Self {
            guest_fds: normalized,
        })
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
        handoff: &Self::Handoff,
        binary_path: &Path,
    ) -> Result<DaemonSpawnSpec> {
        ensure!(
            spec.resolved_interfaces.len() == handoff.guest_fds.len(),
            "vmm handoff guest fd count ({}) must match resolved interface count ({})",
            handoff.guest_fds.len(),
            spec.resolved_interfaces.len()
        );

        let args = encode_launch_spec_args(spec)?;
        let fd_remaps = spec
            .resolved_interfaces
            .iter()
            .zip(&handoff.guest_fds)
            .map(|(interface, guest_fd)| capsa_sandbox::FdRemap {
                source_fd: guest_fd.as_raw_fd(),
                target_fd: interface.guest_fd,
            })
            .collect();

        Ok(DaemonSpawnSpec {
            args,
            sandbox: vmm_sandbox_spec(&spec.vm_config, binary_path),
            fd_remaps,
            stdin_null: false,
        })
    }

    fn readiness(_spec: &Self::Spec, _handoff: &mut Self::Handoff) -> Result<Self::Ready> {
        Ok(NoReadiness)
    }

    fn on_spawned(_spec: &Self::Spec, handoff: &mut Self::Handoff) -> Result<()> {
        handoff.guest_fds.clear();
        Ok(())
    }

    fn on_spawn_failed(_spec: &Self::Spec, _handoff: Self::Handoff) -> Result<()> {
        Ok(())
    }

    fn on_shutdown(_spec: &Self::Spec, _handoff: Self::Handoff) -> Result<()> {
        Ok(())
    }
}

fn duplicate_fd_for_remap(fd: &OwnedFd) -> Result<OwnedFd> {
    // SAFETY: `fcntl(F_DUPFD_CLOEXEC, ..)` duplicates a valid owned fd and
    // returns a new fd number owned by this process.
    let duplicated =
        unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, MIN_REMAP_SOURCE_FD) };

    if duplicated < 0 {
        return Err(std::io::Error::last_os_error()).context("failed to duplicate vmm handoff fd");
    }

    // SAFETY: `duplicated` is a newly created fd from `fcntl` above.
    Ok(unsafe { OwnedFd::from_raw_fd(duplicated) })
}

fn vmm_sandbox_spec(config: &crate::VmConfig, vmm_exe: &Path) -> capsa_sandbox::SandboxSpec {
    let mut spec = capsa_sandbox::SandboxSpec::new()
        .allow_network(false)
        .allow_kvm(true);

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

#[cfg(test)]
mod tests {
    use std::os::fd::{AsRawFd, OwnedFd};
    use std::os::unix::net::UnixDatagram;

    use crate::daemon::traits::DaemonAdapter;
    use crate::{ResolvedNetworkInterface, VmConfig};

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

    fn sample_spec() -> crate::VmmLaunchSpec {
        crate::VmmLaunchSpec {
            vm_config: sample_vm_config(),
            resolved_interfaces: vec![ResolvedNetworkInterface {
                mac: [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee],
                guest_fd: 100,
            }],
        }
    }

    fn sample_guest_fd() -> OwnedFd {
        let (_left, right) = UnixDatagram::pair().expect("socketpair should succeed");
        right.into()
    }

    #[test]
    fn vmm_sandbox_shape_matches_expected_paths_and_network_setting() {
        let spec = sample_spec();
        let handoff = VmmDaemonHandoff::new(vec![sample_guest_fd()]).expect("handoff should build");

        let spawn_spec =
            VmmDaemonAdapter::spawn_spec(&spec, &handoff, std::path::Path::new("/tmp/capsa-vmm"))
                .expect("spawn spec should build");

        assert!(!spawn_spec.sandbox.allow_network);
        assert!(
            spawn_spec.sandbox.allow_kvm,
            "vmm sandbox must request KVM access so libkrun can open /dev/kvm"
        );
        assert!(spawn_spec
            .sandbox
            .read_only_paths
            .contains(&std::path::PathBuf::from("/tmp/capsa-vmm")));
        assert!(spawn_spec
            .sandbox
            .read_only_paths
            .contains(&std::path::PathBuf::from("/tmp/kernel")));
        assert!(spawn_spec
            .sandbox
            .read_only_paths
            .contains(&std::path::PathBuf::from("/tmp/initramfs")));
        assert!(spawn_spec
            .sandbox
            .read_write_paths
            .contains(&std::path::PathBuf::from("/tmp/root")));
    }

    #[test]
    fn vmm_fd_remaps_follow_resolved_interface_targets() {
        let spec = sample_spec();
        let handoff = VmmDaemonHandoff::new(vec![sample_guest_fd()]).expect("handoff should build");

        let spawn_spec =
            VmmDaemonAdapter::spawn_spec(&spec, &handoff, std::path::Path::new("/tmp/capsa-vmm"))
                .expect("spawn spec should build");

        assert!(!spawn_spec.stdin_null);
        assert_eq!(spawn_spec.fd_remaps.len(), 1);
        assert_eq!(spawn_spec.fd_remaps[0].target_fd, 100);
        assert_ne!(spawn_spec.fd_remaps[0].source_fd, 100);
        assert_eq!(spawn_spec.args[0], "--launch-spec-json");
    }

    #[test]
    fn handoff_normalizes_source_fds_away_from_target_range() {
        let original = sample_guest_fd();
        let original_fd = original.as_raw_fd();
        let handoff = VmmDaemonHandoff::new(vec![original]).expect("handoff should build");

        let spawn_spec = VmmDaemonAdapter::spawn_spec(
            &sample_spec(),
            &handoff,
            std::path::Path::new("/tmp/capsa-vmm"),
        )
        .expect("spawn spec should build");

        assert_ne!(spawn_spec.fd_remaps[0].source_fd, original_fd);
    }
}
