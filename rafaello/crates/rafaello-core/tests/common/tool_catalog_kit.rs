//! Helpers for building inputs to `ToolSchemaCatalog::build` in
//! tests (scope §OP2 item 1). Constructs a minimal `BrokerAcl`,
//! single-entry `compiled` map, and `package_dirs` map for a
//! tempdir-backed plugin package.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::compile::{
    CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan, ToolMeta,
};
use rafaello_core::lock::{CanonicalId, LoadPolicy};

pub fn make_compiled(
    canonical: &CanonicalId,
    provider_id: Option<&str>,
    tool_meta: BTreeMap<String, ToolMeta>,
) -> CompiledPlugin {
    CompiledPlugin {
        canonical: canonical.clone(),
        topic_id: rafaello_core::topic_id::derive(&canonical.to_string()),
        entry_absolute: PathBuf::from("/dev/null"),
        filesystem: FilesystemPlan::default(),
        network: NetworkPlan::Deny,
        env: EnvPlan::default(),
        limits: LimitsPlan {
            max_cpu_time: 1,
            max_open_files: 1,
            max_address_space: None,
            max_processes: None,
        },
        subscribe_patterns: Vec::new(),
        publish_topics: Vec::new(),
        auto_subscribes: Vec::new(),
        tool_meta,
        provider_id: provider_id.map(str::to_string),
        load: LoadPolicy::default(),
        flags: CompiledFlags::default(),
    }
}

pub fn make_acl(canonical: &CanonicalId, tools: &[&str], provider_id: Option<&str>) -> BrokerAcl {
    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: rafaello_core::topic_id::derive(&canonical.to_string()),
            publish_topics: Vec::new(),
            subscribe_patterns: Vec::new(),
            auto_subscribes: Vec::new(),
            provider_id: provider_id.map(str::to_string),
        },
    );
    let mut tool_routes = BTreeMap::new();
    for t in tools {
        tool_routes.insert((*t).to_string(), canonical.clone());
    }
    BrokerAcl {
        plugins,
        tool_routes,
        frontends: BTreeMap::new(),
    }
}

pub fn write_openrpc(dir: &Path, body: &str) {
    std::fs::create_dir_all(dir).expect("mkdir package_dir");
    std::fs::write(dir.join("openrpc.json"), body).expect("write openrpc.json");
}

pub fn package_dirs(canonical: &CanonicalId, dir: &Path) -> BTreeMap<CanonicalId, PathBuf> {
    let mut m = BTreeMap::new();
    m.insert(canonical.clone(), dir.to_path_buf());
    m
}

pub fn single_compiled(
    canonical: &CanonicalId,
    provider_id: Option<&str>,
) -> BTreeMap<CanonicalId, CompiledPlugin> {
    let mut m = BTreeMap::new();
    m.insert(
        canonical.clone(),
        make_compiled(canonical, provider_id, BTreeMap::new()),
    );
    m
}
