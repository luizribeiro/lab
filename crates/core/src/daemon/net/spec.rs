use std::collections::HashSet;

use anyhow::{ensure, Result};
use capsa_net::NetworkPolicy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetLaunchSpec {
    /// Inherited fd the daemon should use to signal readiness. Must be an
    /// open writable fd (typically a pipe write end) inherited from the
    /// launcher. Validated to be >= 3 and disjoint from interface fds so
    /// it cannot collide with stdio or any tap fd.
    pub ready_fd: i32,
    #[serde(default)]
    pub interfaces: Vec<NetInterfaceSpec>,
    #[serde(default)]
    pub port_forwards: Vec<(u16, u16)>,
}

impl NetLaunchSpec {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.ready_fd >= 3,
            "invalid ready_fd {}: must be >= 3 (fds 0/1/2 are reserved for stdio)",
            self.ready_fd
        );

        let mut seen_fds = HashSet::new();
        seen_fds.insert(self.ready_fd);

        for (index, interface) in self.interfaces.iter().enumerate() {
            ensure!(
                interface.host_fd >= 3,
                "interface {index}: invalid host_fd {} (must be >= 3)",
                interface.host_fd
            );
            ensure!(
                seen_fds.insert(interface.host_fd),
                "interface {index}: host_fd {} collides with ready_fd or another interface",
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

    fn spec_with(ready_fd: i32, interfaces: Vec<NetInterfaceSpec>) -> NetLaunchSpec {
        NetLaunchSpec {
            ready_fd,
            interfaces,
            port_forwards: vec![],
        }
    }

    #[test]
    fn validate_rejects_low_host_fd() {
        let spec = spec_with(30, vec![sample_interface(2, [0x02, 0, 0, 0, 0, 1])]);

        let err = spec.validate().expect_err("host_fd < 3 should fail");
        assert!(err.to_string().contains("interface 0: invalid host_fd 2"));
    }

    #[test]
    fn validate_rejects_duplicate_host_fd() {
        let spec = spec_with(
            30,
            vec![
                sample_interface(10, [0x02, 0, 0, 0, 0, 1]),
                sample_interface(10, [0x02, 0, 0, 0, 0, 2]),
            ],
        );

        let err = spec.validate().expect_err("duplicate host fd should fail");
        assert!(err.to_string().contains("interface 1: host_fd 10 collides"));
    }

    #[test]
    fn validate_rejects_host_fd_colliding_with_ready_fd() {
        let spec = spec_with(30, vec![sample_interface(30, [0x02, 0, 0, 0, 0, 1])]);

        let err = spec
            .validate()
            .expect_err("host_fd equal to ready_fd should fail");
        assert!(err.to_string().contains("interface 0: host_fd 30 collides"));
    }

    #[test]
    fn validate_rejects_low_ready_fd() {
        let spec = spec_with(2, vec![sample_interface(10, [0x02, 0, 0, 0, 0, 1])]);

        let err = spec.validate().expect_err("ready_fd < 3 should fail");
        assert!(err.to_string().contains("invalid ready_fd 2"));
    }

    #[test]
    fn validate_rejects_zero_mac() {
        let spec = spec_with(30, vec![sample_interface(10, [0; 6])]);

        let err = spec.validate().expect_err("zero mac should fail");
        assert!(err
            .to_string()
            .contains("interface 0: MAC address is all zeros"));
    }

    #[test]
    fn validate_accepts_unique_nonzero_interfaces() {
        let spec = spec_with(
            30,
            vec![
                sample_interface(10, [0x02, 0, 0, 0, 0, 1]),
                sample_interface(11, [0x02, 0, 0, 0, 0, 2]),
            ],
        );

        spec.validate().expect("spec should validate");
    }
}
