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

/// Launcher -> netd JSON contract. Interfaces are attached at
/// runtime via the control socket (see [`ControlRequest`]), so the
/// launch spec only carries daemon-wide parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetLaunchSpec {
    /// Inherited fd the daemon should use to signal readiness. Must
    /// be an open writable fd (typically a pipe write end) inherited
    /// from the launcher. Validated to be >= 3 so it cannot collide
    /// with stdio.
    pub ready_fd: i32,
    /// Inherited `SOCK_SEQPACKET` fd the daemon should use for
    /// runtime control messages (adding interfaces dynamically).
    /// Validated to be >= 3 and disjoint from `ready_fd`.
    #[serde(default)]
    pub control_fd: Option<i32>,
    /// Outbound network policy applied by the daemon's single
    /// gateway stack. Every attached interface shares this policy.
    #[serde(default)]
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

        if let Some(control_fd) = self.control_fd {
            ensure!(
                control_fd >= 3,
                "invalid control_fd {control_fd}: must be >= 3 (fds 0/1/2 are reserved for stdio)"
            );
            ensure!(
                seen_fds.insert(control_fd),
                "control_fd {control_fd} collides with ready_fd"
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
    pub kernel: PathBuf,
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

/// Runtime control request sent from `capsa-core` to a running
/// `capsa-netd` over its control `SOCK_SEQPACKET`. The JSON body
/// travels as the payload; any host-side fd the request references
/// is transferred out of band via a single `SCM_RIGHTS` ancillary
/// message on the same sendmsg call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ControlRequest {
    /// Attach a new guest interface to the running network. The
    /// host-side socketpair fd is transferred out of band via
    /// `SCM_RIGHTS` on the same message.
    AddInterface {
        mac: [u8; 6],
        #[serde(default)]
        port_forwards: Vec<(u16, u16)>,
    },
}

/// Response to a [`ControlRequest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ControlResponse {
    Ok,
    Error { message: String },
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

    fn spec_with(ready_fd: i32) -> NetLaunchSpec {
        NetLaunchSpec {
            ready_fd,
            control_fd: None,
            policy: None,
        }
    }

    #[test]
    fn validate_rejects_low_ready_fd() {
        let err = spec_with(2)
            .validate()
            .expect_err("ready_fd < 3 should fail");
        assert!(err.to_string().contains("invalid ready_fd 2"));
    }

    #[test]
    fn validate_accepts_valid_ready_fd() {
        spec_with(30).validate().expect("spec should validate");
    }

    #[test]
    fn validate_rejects_control_fd_below_three() {
        let mut spec = spec_with(30);
        spec.control_fd = Some(2);
        let err = spec.validate().expect_err("control_fd < 3 should fail");
        assert!(err.to_string().contains("invalid control_fd 2"));
    }

    #[test]
    fn validate_rejects_control_fd_colliding_with_ready_fd() {
        let mut spec = spec_with(30);
        spec.control_fd = Some(30);
        let err = spec
            .validate()
            .expect_err("control_fd equal to ready_fd should fail");
        assert!(err.to_string().contains("control_fd 30 collides"));
    }

    #[test]
    fn validate_accepts_unique_control_fd() {
        let mut spec = spec_with(30);
        spec.control_fd = Some(50);
        spec.validate().expect("unique control_fd should validate");
    }
}

#[cfg(test)]
mod control_tests {
    use super::*;

    #[test]
    fn add_interface_round_trip_preserves_fields() {
        let req = ControlRequest::AddInterface {
            mac: [0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee],
            port_forwards: vec![(8080, 80), (8443, 443)],
        };

        let encoded = serde_json::to_string(&req).expect("request should serialize");
        let decoded: ControlRequest =
            serde_json::from_str(&encoded).expect("request should deserialize");

        assert_eq!(decoded, req);
    }

    #[test]
    fn add_interface_wire_format_is_tagged_lowercase() {
        let req = ControlRequest::AddInterface {
            mac: [0x02, 0, 0, 0, 0, 1],
            port_forwards: vec![],
        };

        let encoded = serde_json::to_string(&req).unwrap();
        assert!(
            encoded.starts_with(r#"{"op":"add_interface""#),
            "unexpected wire format: {encoded}"
        );
    }

    #[test]
    fn add_interface_defaults_port_forwards_when_missing() {
        let encoded = r#"{"op":"add_interface","mac":[2,0,0,0,0,1]}"#;
        let decoded: ControlRequest =
            serde_json::from_str(encoded).expect("missing forwards should default");

        match decoded {
            ControlRequest::AddInterface { port_forwards, .. } => {
                assert!(port_forwards.is_empty());
            }
        }
    }

    #[test]
    fn ok_response_round_trip() {
        let encoded = serde_json::to_string(&ControlResponse::Ok).unwrap();
        assert_eq!(encoded, r#"{"status":"ok"}"#);
        let decoded: ControlResponse = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, ControlResponse::Ok);
    }

    #[test]
    fn error_response_round_trip_preserves_message() {
        let resp = ControlResponse::Error {
            message: "pool exhausted".into(),
        };
        let encoded = serde_json::to_string(&resp).unwrap();
        let decoded: ControlResponse = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, resp);
        assert!(encoded.contains(r#""status":"error""#));
        assert!(encoded.contains("pool exhausted"));
    }
}

#[cfg(test)]
mod vmm_tests {
    use super::*;

    fn base_spec() -> VmmLaunchSpec {
        VmmLaunchSpec {
            kernel: "/tmp/kernel".into(),
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
