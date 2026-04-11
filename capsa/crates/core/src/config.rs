use std::path::PathBuf;

use anyhow::{bail, Result};
use capsa_net::NetworkPolicy;
use serde::{Deserialize, Serialize};

/// Network interface configuration for a VM (user-facing).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmNetworkInterfaceConfig {
    /// MAC address (auto-generated if None).
    #[serde(default)]
    pub mac: Option<[u8; 6]>,
    /// Optional outbound policy for this interface (runtime defaults to deny-all when omitted).
    #[serde(default)]
    pub policy: Option<NetworkPolicy>,
    /// TCP host->guest port forwards as (host_port, guest_port).
    #[serde(default)]
    pub port_forwards: Vec<(u16, u16)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmConfig {
    pub root: Option<PathBuf>,
    pub kernel: Option<PathBuf>,
    pub initramfs: Option<PathBuf>,
    pub kernel_cmdline: Option<String>,
    pub vcpus: u8,
    pub memory_mib: u32,
    pub verbosity: u8,
    /// Network interfaces. Empty = no networking (default).
    #[serde(default)]
    pub interfaces: Vec<VmNetworkInterfaceConfig>,
}

impl VmConfig {
    pub fn validate(&self) -> Result<()> {
        if self.interfaces.len() > 1 {
            bail!("multiple network interfaces are not supported yet");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{VmConfig, VmNetworkInterfaceConfig};
    use capsa_net::{DomainPattern, NetworkPolicy};

    fn sample_vm_config() -> VmConfig {
        VmConfig {
            root: Some("/tmp/root".into()),
            kernel: None,
            initramfs: None,
            kernel_cmdline: None,
            vcpus: 1,
            memory_mib: 512,
            verbosity: 0,
            interfaces: vec![],
        }
    }

    #[test]
    fn vm_config_validate_accepts_single_interface() {
        let mut config = sample_vm_config();
        config.interfaces = vec![VmNetworkInterfaceConfig {
            mac: None,
            policy: None,
            port_forwards: vec![],
        }];
        config.validate().expect("single interface should be valid");
    }

    #[test]
    fn vm_config_validate_rejects_multiple_interfaces() {
        let mut config = sample_vm_config();
        config.interfaces = vec![
            VmNetworkInterfaceConfig {
                mac: None,
                policy: None,
                port_forwards: vec![],
            },
            VmNetworkInterfaceConfig {
                mac: None,
                policy: None,
                port_forwards: vec![],
            },
        ];

        let err = config.validate().expect_err("validation should fail");
        assert!(err
            .to_string()
            .contains("multiple network interfaces are not supported yet"));
    }

    #[test]
    fn vm_network_interface_deserializes_missing_policy_as_none() {
        let iface: VmNetworkInterfaceConfig =
            serde_json::from_str(r#"{"mac":[2,170,187,204,221,238]}"#)
                .expect("interface should deserialize");

        assert_eq!(iface.mac, Some([2, 170, 187, 204, 221, 238]));
        assert_eq!(iface.policy, None);
        assert!(iface.port_forwards.is_empty());
    }

    #[test]
    fn vm_network_interface_roundtrip_preserves_policy() {
        let iface = VmNetworkInterfaceConfig {
            mac: Some([0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee]),
            policy: Some(NetworkPolicy::deny_all().allow_domain(
                DomainPattern::parse("api.example.com").expect("pattern should parse"),
            )),
            port_forwards: vec![(8080, 80)],
        };

        let json = serde_json::to_string(&iface).expect("interface should serialize");
        let roundtrip: VmNetworkInterfaceConfig =
            serde_json::from_str(&json).expect("interface should deserialize");

        assert_eq!(roundtrip, iface);
    }

    #[test]
    fn vm_network_interface_rejects_malformed_policy_pattern_on_deserialize() {
        let json = r#"{
            "mac": [2,170,187,204,221,238],
            "policy": {
                "default_action": "Deny",
                "rules": [
                    {
                        "action": "Allow",
                        "criteria": {
                            "Domain": "*example.com"
                        }
                    }
                ]
            }
        }"#;

        let err = serde_json::from_str::<VmNetworkInterfaceConfig>(json)
            .expect_err("malformed wildcard should fail to deserialize");
        assert!(err.to_string().contains("wildcard host pattern"));
    }
}
