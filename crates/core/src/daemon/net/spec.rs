use std::collections::HashSet;

use anyhow::{ensure, Result};
use capsa_net::NetworkPolicy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetLaunchSpec {
    #[serde(default)]
    pub interfaces: Vec<NetInterfaceSpec>,
}

impl NetLaunchSpec {
    pub fn validate(&self) -> Result<()> {
        let mut seen_fds = HashSet::new();

        for (index, interface) in self.interfaces.iter().enumerate() {
            ensure!(
                interface.host_fd >= 0,
                "interface {index}: invalid host_fd {}",
                interface.host_fd
            );
            ensure!(
                seen_fds.insert(interface.host_fd),
                "interface {index}: duplicate host_fd {}",
                interface.host_fd
            );
            ensure!(
                interface.mac != [0u8; 6],
                "interface {index}: MAC address is all zeros"
            );
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetInterfaceSpec {
    pub host_fd: i32,
    pub mac: [u8; 6],
    pub policy: Option<NetworkPolicy>,
}

#[cfg(test)]
mod tests {
    use super::{NetInterfaceSpec, NetLaunchSpec};

    fn sample_interface(host_fd: i32, mac: [u8; 6]) -> NetInterfaceSpec {
        NetInterfaceSpec {
            host_fd,
            mac,
            policy: None,
        }
    }

    #[test]
    fn validate_rejects_negative_host_fd() {
        let spec = NetLaunchSpec {
            interfaces: vec![sample_interface(-1, [0x02, 0, 0, 0, 0, 1])],
        };

        let err = spec.validate().expect_err("negative host fd should fail");
        assert!(err.to_string().contains("interface 0: invalid host_fd -1"));
    }

    #[test]
    fn validate_rejects_duplicate_host_fd() {
        let spec = NetLaunchSpec {
            interfaces: vec![
                sample_interface(10, [0x02, 0, 0, 0, 0, 1]),
                sample_interface(10, [0x02, 0, 0, 0, 0, 2]),
            ],
        };

        let err = spec.validate().expect_err("duplicate host fd should fail");
        assert!(err
            .to_string()
            .contains("interface 1: duplicate host_fd 10"));
    }

    #[test]
    fn validate_rejects_zero_mac() {
        let spec = NetLaunchSpec {
            interfaces: vec![sample_interface(10, [0; 6])],
        };

        let err = spec.validate().expect_err("zero mac should fail");
        assert!(err
            .to_string()
            .contains("interface 0: MAC address is all zeros"));
    }

    #[test]
    fn validate_accepts_unique_nonzero_interfaces() {
        let spec = NetLaunchSpec {
            interfaces: vec![
                sample_interface(10, [0x02, 0, 0, 0, 0, 1]),
                sample_interface(11, [0x02, 0, 0, 0, 0, 2]),
            ],
        };

        spec.validate().expect("spec should validate");
    }
}
