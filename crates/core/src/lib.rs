mod config;
#[cfg(target_os = "linux")]
mod proc;
mod start;

pub use capsa_net::{
    DomainPattern, DomainPatternParseError, MatchCriteria, NetworkPolicy, PolicyAction, PolicyRule,
};
pub use config::{VmConfig, VmNetworkInterfaceConfig};

#[cfg(all(test, target_os = "linux"))]
pub(crate) fn test_env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}
