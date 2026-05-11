//! `.grant` — bundle-aware grant snapshot per scope §L3.
//!
//! Capability path fields are raw `String` at parse time per
//! pi-2 commits-finding 6 + the c13 phase note: lock-side
//! capability path validation lives in V3 (c25) with
//! `ValidationError::LockCapabilityPathRelative`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::manifest::capabilities::NetworkMode;

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bundles: BTreeMap<String, GrantBundle>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subscribes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub publishes: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GrantBundle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filesystem: Option<GrantFilesystem>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<GrantNetwork>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<GrantEnv>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<GrantLimits>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GrantFilesystem {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_dirs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub write_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub write_dirs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exec_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exec_dirs: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GrantNetwork {
    #[serde(default)]
    pub mode: NetworkMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow_hosts: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GrantEnv {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pass: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub set: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow_secrets: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GrantLimits {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cpu_time: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_open_files: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_address_space: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_processes: Option<u64>,
}
