mod attach;
mod boot;
mod error;
mod network;

pub use self::attach::{Attachable, NetworkAttach};
pub use self::boot::{Boot, KernelBoot};
pub use self::error::BuildError;
pub use self::network::{Network, NetworkBuilder};

pub use capsa_core::{
    DomainPattern, DomainPatternParseError, MatchCriteria, NetworkPolicy, PolicyAction, PolicyRule,
    VmConfig, VmNetworkInterfaceConfig,
};
