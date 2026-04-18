pub(crate) mod config;
mod dns;
pub mod frame;
mod gateway;
mod nat;
pub mod policy;
pub mod switch;
mod util;

pub use gateway::{
    GatewayStack, GatewayStackConfig, LeasePreallocationError, LeasePreallocator,
    PortForwardRequest,
};
pub use policy::{
    DomainPattern, DomainPatternParseError, MatchCriteria, NetworkPolicy, PacketInfo, PolicyAction,
    PolicyChecker, PolicyResult, PolicyRule, TransportProtocol,
};
pub use switch::{SwitchPort, VirtualSwitch};

pub use switch::bridge::bridge_to_switch;
pub use switch::socketpair::SocketPairDevice;

/// Paths that the capsa-net library and its tokio runtime open at
/// startup or during normal operation. Production sandboxes around
/// capsa-netd must allow read access to these so the DNS proxy and
/// the runtime probes don't hit EPERM.
///
/// This is intentionally a single list per platform rather than a
/// "what came from where" breakdown: from the sandbox policy's
/// perspective, the daemon is one process with one allowlist, and
/// the integration tests verify the whole thing as a unit.
pub fn runtime_read_paths() -> &'static [&'static str] {
    RUNTIME_READ_PATHS
}

#[cfg(target_os = "linux")]
const RUNTIME_READ_PATHS: &[&str] = &[
    "/etc/resolv.conf",
    "/proc/self/cgroup",
    "/proc/stat",
    "/sys/devices/system/cpu/online",
];

#[cfg(target_os = "macos")]
const RUNTIME_READ_PATHS: &[&str] = &["/etc/resolv.conf"];

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
const RUNTIME_READ_PATHS: &[&str] = &[];
