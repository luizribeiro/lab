//! Cross-consumer networking policy library.
//!
//! `outpost` owns the host-allowlist DSL used by both capsa's VM
//! sandbox and lockin's process sandbox. The policy vocabulary lives
//! here; enforcement backends (packet filtering in capsa-vmnet, HTTP
//! CONNECT proxy in outpost-proxy) live alongside and consume these
//! types.

pub mod policy;

pub use policy::{
    DomainPattern, DomainPatternParseError, MatchCriteria, NetworkPolicy, PolicyAction, PolicyRule,
};
