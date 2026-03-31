pub(crate) mod config;
mod dns;
pub mod frame;
mod gateway;
mod nat;
pub mod policy;
pub mod switch;
mod util;

pub use gateway::{GatewayStack, GatewayStackConfig, PortForwardRequest};
pub use policy::{
    DomainPattern, DomainPatternParseError, MatchCriteria, NetworkPolicy, PacketInfo, PolicyAction,
    PolicyChecker, PolicyResult, PolicyRule, TransportProtocol,
};
pub use switch::{SwitchPort, VirtualSwitch};

pub use switch::bridge::bridge_to_switch;
pub use switch::socketpair::SocketPairDevice;
