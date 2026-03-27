mod boot;
mod config;
pub mod daemon;
mod launcher;
mod libkrun;
mod runtime;

pub use capsa_net::{
    DomainPattern, DomainPatternParseError, MatchCriteria, NetworkPolicy, PolicyAction, PolicyRule,
};
pub use config::{VmConfig, VmNetworkInterfaceConfig};
pub use daemon::vmm::spec::{ResolvedNetworkInterface, VmmLaunchSpec};

#[cfg(test)]
pub(crate) fn test_env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

#[doc(hidden)]
pub use runtime::start_vm;
