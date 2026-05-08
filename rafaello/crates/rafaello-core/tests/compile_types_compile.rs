//! Build-only assertion for the c28 compile module skeleton: the
//! public types are reachable from the crate root and have the
//! shape documented in scope §C1.

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::compile::{
    CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan, ToolMeta,
};
use rafaello_core::lock::canonical_id::CanonicalId;
use rafaello_core::lock::load_policy::LoadPolicy;

#[allow(dead_code)]
fn build_compiled_plugin(canonical: CanonicalId) -> CompiledPlugin {
    CompiledPlugin {
        canonical,
        topic_id: String::new(),
        entry_absolute: PathBuf::new(),
        filesystem: FilesystemPlan {
            read_paths: Vec::<PathBuf>::new(),
            read_dirs: Vec::<PathBuf>::new(),
            write_paths: Vec::<PathBuf>::new(),
            write_dirs: Vec::<PathBuf>::new(),
            exec_paths: Vec::<PathBuf>::new(),
            exec_dirs: Vec::<PathBuf>::new(),
        },
        network: NetworkPlan::Proxy {
            allow_hosts: Vec::<String>::new(),
        },
        env: EnvPlan {
            pass: Vec::<String>::new(),
            set: BTreeMap::<String, String>::new(),
        },
        limits: LimitsPlan {
            max_cpu_time: 0,
            max_open_files: 0,
            max_address_space: None,
            max_processes: None,
        },
        subscribe_patterns: Vec::<String>::new(),
        publish_topics: Vec::<String>::new(),
        auto_subscribes: Vec::<String>::new(),
        tool_meta: {
            let mut m = BTreeMap::<String, ToolMeta>::new();
            m.insert(
                String::new(),
                ToolMeta {
                    sinks: Vec::<String>::new(),
                    sinks_inferred: false,
                    grant_match: None::<PathBuf>,
                    always_confirm: false,
                },
            );
            m
        },
        provider_id: None::<String>,
        load: LoadPolicy::default(),
        flags: CompiledFlags {
            i_know_what_im_doing: false,
            allow_credential_paths: false,
        },
    }
}

#[allow(dead_code)]
fn network_variants() -> [NetworkPlan; 3] {
    [
        NetworkPlan::Deny,
        NetworkPlan::AllowAll,
        NetworkPlan::Proxy {
            allow_hosts: Vec::new(),
        },
    ]
}

#[test]
fn types_are_reachable() {
    let _ = build_compiled_plugin as fn(CanonicalId) -> CompiledPlugin;
    let _ = network_variants as fn() -> [NetworkPlan; 3];
}
