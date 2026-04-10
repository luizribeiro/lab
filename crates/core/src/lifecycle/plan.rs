//! Typed phases an interface passes through on the way from
//! `VmConfig` to `capsa-vmm`'s launch spec, plus the MAC and path
//! helpers shared by `netd.rs` and `vmm.rs`.
//!
//! ```text
//!   VmConfig.interfaces
//!     │  plan_interfaces           ← read config, resolve MACs
//!     ▼
//!   Vec<InterfacePlan>
//!     │  open_interface_sockets    ← allocate UnixDatagram pairs
//!     ▼
//!   Vec<InterfaceSockets>
//!     │  netd::spawn_netd          ← inherit host_fd into netd sandbox,
//!     │                              hand the guest end to the VMM caller
//!     ▼
//!   Vec<VmmInterfaceBinding>
//!     │  vmm::spawn_vmm             ← inherit guest_fd into vmm sandbox,
//!     │                               record the kernel-assigned raw fd
//!     ▼
//!   Vec<ResolvedNetworkInterface>
//! ```
//!
//! No placeholder values exist along the way: each transition
//! produces a fully-populated value for its layer.

use std::os::fd::OwnedFd;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use capsa_net::NetworkPolicy;

use crate::config::{VmConfig, VmNetworkInterfaceConfig};

/// Per-interface configuration after MAC resolution; the first
/// typed phase a `VmConfig` interface enters. No fds yet.
pub(super) struct InterfacePlan {
    pub(super) mac: [u8; 6],
    pub(super) policy: Option<NetworkPolicy>,
    pub(super) port_forwards: Vec<(u16, u16)>,
}

/// An `InterfacePlan` that has been bound to a freshly-allocated
/// `UnixDatagram` pair. The host end goes to capsa-netd; the guest
/// end goes to capsa-vmm. Owned because we hand the two halves
/// across two separate sandbox builders.
pub(super) struct InterfaceSockets {
    pub(super) plan: InterfacePlan,
    pub(super) host_fd: OwnedFd,
    pub(super) guest_fd: OwnedFd,
}

/// The guest-side socket fd and MAC for one interface after netd
/// has consumed the host end.
pub(super) struct VmmInterfaceBinding {
    pub(super) mac: [u8; 6],
    pub(super) guest_fd: OwnedFd,
}

pub(super) fn plan_interfaces(config: &VmConfig) -> Result<Vec<InterfacePlan>> {
    config
        .interfaces
        .iter()
        .enumerate()
        .map(|(index, iface)| {
            Ok(InterfacePlan {
                mac: resolve_mac(iface, index).with_context(|| format!("interface {index}"))?,
                policy: iface.policy.clone(),
                port_forwards: iface.port_forwards.clone(),
            })
        })
        .collect()
}

pub(super) fn open_interface_sockets(plans: Vec<InterfacePlan>) -> Result<Vec<InterfaceSockets>> {
    plans
        .into_iter()
        .enumerate()
        .map(|(index, plan)| {
            let (host_sock, guest_sock) = UnixDatagram::pair()
                .with_context(|| format!("failed to create socketpair for interface {index}"))?;
            Ok(InterfaceSockets {
                plan,
                host_fd: host_sock.into(),
                guest_fd: guest_sock.into(),
            })
        })
        .collect()
}

pub(super) fn resolve_mac(iface: &VmNetworkInterfaceConfig, index: usize) -> Result<[u8; 6]> {
    match iface.mac {
        Some(mac) => {
            ensure!(mac != [0u8; 6], "interface MAC address is all zeros");
            Ok(mac)
        }
        None => Ok(generate_mac(index)),
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
        let err = resolve_mac(&iface, 0).expect_err("zero mac should be rejected");
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
        assert_eq!(resolve_mac(&iface, 0).unwrap(), explicit);
    }
}
