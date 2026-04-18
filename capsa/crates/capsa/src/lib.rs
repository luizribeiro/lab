mod error;

pub use self::error::BuildError;

pub use capsa_core::{
    DomainPattern, DomainPatternParseError, MatchCriteria, NetworkPolicy, PolicyAction, PolicyRule,
    VmConfig, VmNetworkInterfaceConfig,
};
