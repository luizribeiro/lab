//! `[capabilities]` block raw decode (scope §M5, parse half).
//!
//! Per the m1-manifest phase boundary, this commit decodes the
//! bundle-and-section structure only. Bundle-key resolution
//! (`default | <tool-name>` cross-ref to `provides.tools`),
//! the `network.allow_hosts`-vs-`mode` rule, and the
//! `exec_paths`-inside-project check are deferred to V1
//! (`validate::manifest_standalone`, c10). Path fields run
//! `CapabilityPathTemplate::parse` at decode time, so shape
//! errors (control chars, backslash, bare relative, unknown
//! placeholder) surface here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::manifest::capability_path_template::CapabilityPathTemplate;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct CapabilityBundle {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filesystem: Option<FilesystemCapabilities>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkCapabilities>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<EnvCapabilities>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<LimitsCapabilities>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct FilesystemCapabilities {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_paths: Vec<CapabilityPathTemplate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_dirs: Vec<CapabilityPathTemplate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub write_paths: Vec<CapabilityPathTemplate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub write_dirs: Vec<CapabilityPathTemplate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exec_paths: Vec<CapabilityPathTemplate>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exec_dirs: Vec<CapabilityPathTemplate>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct NetworkCapabilities {
    #[serde(default)]
    pub mode: NetworkMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow_hosts: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum NetworkMode {
    #[default]
    Deny,
    Proxy,
    AllowAll,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct EnvCapabilities {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pass: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub set: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(deny_unknown_fields)]
pub struct LimitsCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_cpu_time: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_open_files: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_address_space: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_processes: Option<u64>,
}

pub type Capabilities = BTreeMap<String, CapabilityBundle>;
