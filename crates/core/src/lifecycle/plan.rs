//! Config → spec shaping. Pure functions that turn a `VmConfig`
//! into the values the spawn paths need: a canonicalized
//! `VmmLaunchSpec`, a resolved MAC for the network interface, and
//! the `canonical_or_unchanged` helper used by both daemon sandbox
//! builders.

use std::path::{Path, PathBuf};

use anyhow::{ensure, Result};
use capsa_spec::{ResolvedNetworkInterface, VmmLaunchSpec};

use crate::config::{VmConfig, VmNetworkInterfaceConfig};

pub(super) fn build_vmm_spec(
    config: &VmConfig,
    resolved_interfaces: Vec<ResolvedNetworkInterface>,
) -> VmmLaunchSpec {
    // Canonicalize before encoding into the launch spec so the
    // child VMM opens the same on-disk path the sandbox policy
    // will allow. Without this, `/tmp/...` and `/private/tmp/...`
    // diverge on darwin and the open hits EPERM.
    VmmLaunchSpec {
        root: config.root.as_deref().map(canonical_or_unchanged),
        kernel: config.kernel.as_deref().map(canonical_or_unchanged),
        initramfs: config.initramfs.as_deref().map(canonical_or_unchanged),
        kernel_cmdline: config.kernel_cmdline.clone(),
        vcpus: config.vcpus,
        memory_mib: config.memory_mib,
        verbosity: config.verbosity,
        resolved_interfaces,
    }
}

pub(super) fn resolve_mac(iface: &VmNetworkInterfaceConfig) -> Result<[u8; 6]> {
    match iface.mac {
        Some(mac) => {
            ensure!(mac != [0u8; 6], "interface MAC address is all zeros");
            Ok(mac)
        }
        None => Ok(generate_mac(0)),
    }
}

fn generate_mac(index: usize) -> [u8; 6] {
    let mut seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    seed ^= (std::process::id() as u128) << 32;
    seed ^= index as u128;

    let mut mac = [0u8; 6];
    mac[0] = 0x02; // locally administered, unicast
    mac[1] = ((seed >> 8) & 0xff) as u8;
    mac[2] = ((seed >> 16) & 0xff) as u8;
    mac[3] = ((seed >> 24) & 0xff) as u8;
    mac[4] = ((seed >> 32) & 0xff) as u8;
    mac[5] = ((seed >> 40) & 0xff) as u8;

    if mac == [0u8; 6] {
        mac[5] = 1;
    }

    mac
}

/// Resolves symlinks before a path is handed across a process
/// boundary so capsa-vmm and the darwin sandbox-exec policy
/// agree on the post-resolution path the kernel will see at
/// `open(2)` time. Falls back to the input for paths that
/// don't exist (test fixtures, etc.).
pub(super) fn canonical_or_unchanged(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_mac_is_non_zero() {
        assert_ne!(generate_mac(0), [0u8; 6]);
    }

    #[test]
    fn resolve_mac_rejects_explicit_zero() {
        let iface = VmNetworkInterfaceConfig {
            mac: Some([0; 6]),
            policy: None,
            port_forwards: vec![],
        };
        let err = resolve_mac(&iface).expect_err("zero mac should be rejected");
        assert!(err.to_string().contains("MAC address is all zeros"));
    }

    #[test]
    fn resolve_mac_passes_through_explicit_nonzero() {
        let explicit = [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee];
        let iface = VmNetworkInterfaceConfig {
            mac: Some(explicit),
            policy: None,
            port_forwards: vec![],
        };
        assert_eq!(resolve_mac(&iface).unwrap(), explicit);
    }
}
