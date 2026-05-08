//! Compile-side plan types per scope §C1.
//!
//! `CompiledPlugin` is the structured plan m1 hands m2's plugin
//! supervisor; m2 picks the application order. The `compile_plugin`
//! entry point lands in c29 — this module currently exposes the
//! data types only.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::lock::canonical_id::CanonicalId;
use crate::lock::load_policy::LoadPolicy;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledPlugin {
    pub canonical: CanonicalId,
    pub topic_id: String,
    pub entry_absolute: PathBuf,
    pub filesystem: FilesystemPlan,
    pub network: NetworkPlan,
    pub env: EnvPlan,
    pub limits: LimitsPlan,
    pub subscribe_patterns: Vec<String>,
    pub publish_topics: Vec<String>,
    pub auto_subscribes: Vec<String>,
    pub tool_meta: BTreeMap<String, ToolMeta>,
    pub provider_id: Option<String>,
    pub load: LoadPolicy,
    pub flags: CompiledFlags,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilesystemPlan {
    pub read_paths: Vec<PathBuf>,
    pub read_dirs: Vec<PathBuf>,
    pub write_paths: Vec<PathBuf>,
    pub write_dirs: Vec<PathBuf>,
    pub exec_paths: Vec<PathBuf>,
    pub exec_dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkPlan {
    Deny,
    AllowAll,
    Proxy { allow_hosts: Vec<String> },
}

impl Default for NetworkPlan {
    fn default() -> Self {
        NetworkPlan::Deny
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvPlan {
    pub pass: Vec<String>,
    pub set: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LimitsPlan {
    pub max_cpu_time: u64,
    pub max_open_files: u64,
    pub max_address_space: Option<u64>,
    pub max_processes: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompiledFlags {
    pub i_know_what_im_doing: bool,
    pub allow_credential_paths: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolMeta {
    pub sinks: Vec<String>,
    pub sinks_inferred: bool,
    pub grant_match: Option<PathBuf>,
    pub always_confirm: bool,
}
