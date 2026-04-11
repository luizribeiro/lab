//! Shared launch-spec contract between `capsa-core` and the daemon binaries
//! (`capsa-netd`, `capsa-vmm`).
//!
//! This crate intentionally contains only data types, their validation, and
//! the thin argv encode/parse helpers. It has no knowledge of sandboxing,
//! process spawning, or runtime policy.

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::{bail, ensure, Context, Result};
use capsa_net::NetworkPolicy;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

pub const LAUNCH_SPEC_JSON_FLAG: &str = "--launch-spec-json";
const USAGE: &str = "usage: --launch-spec-json <json>";

pub fn encode_launch_spec_args<T: Serialize>(spec: &T) -> Result<Vec<String>> {
    let launch_spec_json =
        serde_json::to_string(spec).context("failed to serialize launch spec")?;
    Ok(vec![LAUNCH_SPEC_JSON_FLAG.to_string(), launch_spec_json])
}

pub fn parse_launch_spec_args<T, I, S>(args: I) -> Result<T>
where
    T: DeserializeOwned,
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);

    let flag = args.next();
    let launch_spec_json = args.next();

    if flag.as_deref() != Some(LAUNCH_SPEC_JSON_FLAG)
        || launch_spec_json.is_none()
        || args.next().is_some()
    {
        bail!(USAGE);
    }

    serde_json::from_str(
        launch_spec_json
            .as_deref()
            .expect("checked above: launch spec json is present"),
    )
    .context("failed to parse launch spec JSON")
}

/// Launcher -> netd JSON contract.
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetInterfaceSpec {
    pub host_fd: i32,
    pub mac: [u8; 6],
    pub policy: Option<NetworkPolicy>,
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

/// Launcher -> vmm JSON contract.
///
/// Carries only the boot, resource, and resolved-interface information the
/// vmm actually needs. The user-facing `VmConfig` lives in `capsa-core` and
/// never crosses this boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmmLaunchSpec {
    pub root: Option<PathBuf>,
    pub kernel: Option<PathBuf>,
    pub initramfs: Option<PathBuf>,
    pub kernel_cmdline: Option<String>,
    pub vcpus: u8,
    pub memory_mib: u32,
    pub verbosity: u8,
    #[serde(default)]
    pub resolved_interfaces: Vec<ResolvedNetworkInterface>,
}

/// Resolved network interface with launcher-assigned fd.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedNetworkInterface {
    /// MAC address (always populated, non-zero).
    pub mac: [u8; 6],
    /// FD number in the vmm process (inherited from launcher). Must be >= 3
    /// (fds 0/1/2 are reserved for stdio).
    pub guest_fd: i32,
}

impl VmmLaunchSpec {
    pub fn validate(&self) -> Result<()> {
        let mut seen_fds = HashSet::new();
        for (index, interface) in self.resolved_interfaces.iter().enumerate() {
            ensure!(
                interface.guest_fd >= 3,
                "interface {index}: invalid guest_fd {} (must be >= 3)",
                interface.guest_fd
            );
            ensure!(
                seen_fds.insert(interface.guest_fd),
                "interface {index}: duplicate guest_fd {}",
                interface.guest_fd
            );
            ensure!(
                interface.mac != [0u8; 6],
                "interface {index}: MAC address is all zeros"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
mod args_tests {
    use super::*;

    #[derive(Debug, serde::Serialize, serde::Deserialize, PartialEq)]
    struct DummySpec {
        answer: u32,
    }

    #[test]
    fn parse_accepts_valid_input() {
        let parsed: DummySpec =
            parse_launch_spec_args(vec![LAUNCH_SPEC_JSON_FLAG, "{\"answer\":42}"])
                .expect("valid args should parse");
        assert_eq!(parsed.answer, 42);
    }

    #[test]
    fn parse_rejects_usage_errors() {
        for args in [
            vec![],
            vec!["--wrong-flag"],
            vec![LAUNCH_SPEC_JSON_FLAG],
            vec![LAUNCH_SPEC_JSON_FLAG, "{}", "extra"],
        ] {
            let err = parse_launch_spec_args::<DummySpec, _, _>(args)
                .expect_err("usage errors should fail");
            assert_eq!(err.to_string(), USAGE);
        }
    }

    #[test]
    fn encode_and_parse_round_trip() {
        let expected = DummySpec { answer: 7 };
        let encoded = encode_launch_spec_args(&expected).expect("encoding should succeed");
        let decoded: DummySpec =
            parse_launch_spec_args(encoded).expect("round-trip parse should succeed");
        assert_eq!(decoded, expected);
    }
}

#[cfg(test)]
mod net_tests {
    use super::*;

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

#[cfg(test)]
mod vmm_tests {
    use super::*;

    fn base_spec() -> VmmLaunchSpec {
        VmmLaunchSpec {
            root: Some("/tmp/root".into()),
            kernel: None,
            initramfs: None,
            kernel_cmdline: None,
            vcpus: 1,
            memory_mib: 512,
            verbosity: 0,
            resolved_interfaces: vec![],
        }
    }

    #[test]
    fn validate_accepts_empty_interfaces() {
        base_spec()
            .validate()
            .expect("empty interfaces should validate");
    }

    #[test]
    fn validate_rejects_low_guest_fd() {
        let mut spec = base_spec();
        spec.resolved_interfaces.push(ResolvedNetworkInterface {
            mac: [0x02, 0, 0, 0, 0, 1],
            guest_fd: 2,
        });
        let err = spec.validate().expect_err("guest_fd < 3 should fail");
        assert!(err.to_string().contains("interface 0: invalid guest_fd 2"));
    }

    #[test]
    fn validate_rejects_duplicate_guest_fd() {
        let mut spec = base_spec();
        spec.resolved_interfaces.push(ResolvedNetworkInterface {
            mac: [0x02, 0, 0, 0, 0, 1],
            guest_fd: 10,
        });
        spec.resolved_interfaces.push(ResolvedNetworkInterface {
            mac: [0x02, 0, 0, 0, 0, 2],
            guest_fd: 10,
        });
        let err = spec.validate().expect_err("duplicate guest_fd should fail");
        assert!(err
            .to_string()
            .contains("interface 1: duplicate guest_fd 10"));
    }

    #[test]
    fn validate_rejects_all_zero_mac() {
        let mut spec = base_spec();
        spec.resolved_interfaces.push(ResolvedNetworkInterface {
            mac: [0; 6],
            guest_fd: 10,
        });
        let err = spec.validate().expect_err("zero mac should fail");
        assert!(err
            .to_string()
            .contains("interface 0: MAC address is all zeros"));
    }

    #[test]
    fn validate_accepts_unique_nonzero_interfaces() {
        let mut spec = base_spec();
        spec.resolved_interfaces.push(ResolvedNetworkInterface {
            mac: [0x02, 0, 0, 0, 0, 1],
            guest_fd: 10,
        });
        spec.resolved_interfaces.push(ResolvedNetworkInterface {
            mac: [0x02, 0, 0, 0, 0, 2],
            guest_fd: 11,
        });
        spec.validate().expect("spec should validate");
    }
}
