mod config;
mod lifecycle;

pub use capsa_net::{
    DomainPattern, DomainPatternParseError, MatchCriteria, NetworkPolicy, PolicyAction, PolicyRule,
};
pub use config::VmConfig;
pub use lifecycle::{NetworkProcesses, VmAttachment, VmProcesses};

#[cfg(all(test, any(target_os = "linux", target_os = "macos")))]
pub(crate) fn test_env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}
