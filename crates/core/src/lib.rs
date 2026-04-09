mod config;
pub mod daemon;
mod launcher;
// `proc` lands in commit 7 of the rewrite plan and gets wired into
// `VmConfig::start` in commit 8. Items are unused until then.
#[allow(dead_code)]
pub(crate) mod proc;

pub use capsa_net::{
    DomainPattern, DomainPatternParseError, MatchCriteria, NetworkPolicy, PolicyAction, PolicyRule,
};
pub use capsa_spec::{ResolvedNetworkInterface, VmmLaunchSpec};
pub use config::{VmConfig, VmNetworkInterfaceConfig};

#[cfg(test)]
pub(crate) fn test_env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}
