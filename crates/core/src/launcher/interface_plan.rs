use std::os::fd::{FromRawFd, IntoRawFd, OwnedFd};
use std::os::unix::net::UnixDatagram;

use anyhow::{ensure, Context, Result};
use capsa_net::NetworkPolicy;

use crate::{ResolvedNetworkInterface, VmNetworkInterfaceConfig};

#[derive(Debug)]
pub(crate) struct InterfacePlan {
    pub(crate) interfaces: Vec<PlannedInterface>,
}

#[derive(Debug)]
pub(crate) struct PlannedInterface {
    pub(crate) mac: [u8; 6],
    pub(crate) policy: NetworkPolicy,
    pub(crate) host_fd: OwnedFd,
    pub(crate) guest_fd: OwnedFd,
}

pub(crate) fn build_interface_plan(
    interfaces: &[VmNetworkInterfaceConfig],
) -> Result<InterfacePlan> {
    let mut planned = Vec::with_capacity(interfaces.len());

    for (index, interface) in interfaces.iter().enumerate() {
        let (host_fd, guest_fd) = create_unix_dgram_socketpair()
            .with_context(|| format!("failed to create socketpair for interface {index}"))?;

        planned.push(PlannedInterface {
            mac: resolve_interface_mac(index, interface)?,
            policy: effective_interface_policy(interface),
            host_fd,
            guest_fd,
        });
    }

    Ok(InterfacePlan {
        interfaces: planned,
    })
}

pub(crate) fn create_unix_dgram_socketpair() -> Result<(OwnedFd, OwnedFd)> {
    let (left, right) =
        UnixDatagram::pair().context("failed to create unix datagram socketpair")?;

    let left_raw = left.into_raw_fd();
    let right_raw = right.into_raw_fd();

    // SAFETY: `left_raw` and `right_raw` come from `into_raw_fd`, transferring
    // ownership to the newly created `OwnedFd`s.
    let left_owned = unsafe { OwnedFd::from_raw_fd(left_raw) };
    // SAFETY: same as above for the second socket endpoint.
    let right_owned = unsafe { OwnedFd::from_raw_fd(right_raw) };

    Ok((left_owned, right_owned))
}

pub(crate) fn resolve_interface_mac(
    index: usize,
    interface: &VmNetworkInterfaceConfig,
) -> Result<[u8; 6]> {
    match interface.mac {
        Some(mac) => {
            ensure!(mac != [0; 6], "interface {index}: MAC address is all zeros");
            Ok(mac)
        }
        None => Ok(generate_mac(index)),
    }
}

pub(crate) fn effective_interface_policy(interface: &VmNetworkInterfaceConfig) -> NetworkPolicy {
    interface
        .policy
        .clone()
        .unwrap_or_else(NetworkPolicy::deny_all)
}

pub(crate) fn generate_mac(index: usize) -> [u8; 6] {
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

pub(crate) fn resolved_interfaces_for_plan(
    plan: &[PlannedInterface],
) -> Vec<ResolvedNetworkInterface> {
    // The `guest_fd` field here is a placeholder; the VMM daemon
    // adapter fills it in with the real kernel-assigned fd number
    // when it drains the handoff in `spawn_spec`.
    plan.iter()
        .map(|interface| ResolvedNetworkInterface {
            mac: interface.mac,
            guest_fd: 0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        build_interface_plan, effective_interface_policy, resolve_interface_mac,
        resolved_interfaces_for_plan,
    };
    use crate::VmNetworkInterfaceConfig;
    use capsa_net::{DomainPattern, NetworkPolicy};

    #[test]
    fn generated_mac_is_non_zero() {
        let iface = VmNetworkInterfaceConfig {
            mac: None,
            policy: None,
            port_forwards: vec![],
        };

        let mac = resolve_interface_mac(0, &iface).expect("mac should resolve");

        assert_ne!(mac, [0; 6]);
    }

    #[test]
    fn explicit_mac_is_preserved() {
        let explicit_mac = [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee];
        let iface = VmNetworkInterfaceConfig {
            mac: Some(explicit_mac),
            policy: None,
            port_forwards: vec![],
        };

        let mac = resolve_interface_mac(0, &iface).expect("mac should resolve");

        assert_eq!(mac, explicit_mac);
    }

    #[test]
    fn invalid_mac_is_rejected() {
        let iface = VmNetworkInterfaceConfig {
            mac: Some([0; 6]),
            policy: None,
            port_forwards: vec![],
        };

        let err = resolve_interface_mac(0, &iface).expect_err("all-zero MAC should fail");
        assert!(err.to_string().contains("MAC address is all zeros"));
    }

    #[test]
    fn policy_fallback_is_deny_all_when_omitted() {
        let iface = VmNetworkInterfaceConfig {
            mac: None,
            policy: None,
            port_forwards: vec![],
        };

        assert_eq!(
            effective_interface_policy(&iface),
            NetworkPolicy::deny_all()
        );
    }

    #[test]
    fn resolved_interfaces_preserve_mac_from_plan() {
        let explicit_mac = [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee];
        let interfaces = vec![VmNetworkInterfaceConfig {
            mac: Some(explicit_mac),
            policy: None,
            port_forwards: vec![],
        }];
        let plan = build_interface_plan(&interfaces).expect("plan should build");

        let resolved = resolved_interfaces_for_plan(&plan.interfaces);

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].mac, explicit_mac);
        // guest_fd is a placeholder here; the VMM adapter fills in
        // the real kernel-assigned number at spawn_spec time.
    }

    #[test]
    fn explicit_interface_policy_is_preserved() {
        let explicit_policy = NetworkPolicy::deny_all()
            .allow_domain(DomainPattern::parse("api.example.com").expect("pattern should parse"));
        let iface = VmNetworkInterfaceConfig {
            mac: None,
            policy: Some(explicit_policy.clone()),
            port_forwards: vec![],
        };

        assert_eq!(effective_interface_policy(&iface), explicit_policy);
    }
}
