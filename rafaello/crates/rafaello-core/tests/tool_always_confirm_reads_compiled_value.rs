//! c09 — §Si3. `tool_always_confirm` reads through to the compiled
//! `ToolMeta.always_confirm` value; both `true` and `false` manifests
//! round-trip.

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::compile::{
    CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan, ToolMeta,
};
use rafaello_core::lock::CanonicalId;

fn plugin_with_confirm(tool: &str, always_confirm: bool) -> CompiledPlugin {
    let mut tool_meta = BTreeMap::new();
    tool_meta.insert(
        tool.to_owned(),
        ToolMeta {
            sinks: Vec::new(),
            sinks_inferred: false,
            grant_match: None,
            always_confirm,
        },
    );
    CompiledPlugin {
        canonical: CanonicalId::parse("github.com/acme:t@1.0.0").unwrap(),
        topic_id: "t".to_owned(),
        entry_absolute: PathBuf::from("/dev/null"),
        filesystem: FilesystemPlan::default(),
        network: NetworkPlan::default(),
        env: EnvPlan::default(),
        limits: LimitsPlan::default(),
        subscribe_patterns: Vec::new(),
        publish_topics: Vec::new(),
        auto_subscribes: Vec::new(),
        tool_meta,
        provider_id: None,
        load: Default::default(),
        flags: CompiledFlags::default(),
    }
}

#[test]
fn tool_always_confirm_true_round_trips() {
    let plan = plugin_with_confirm("send-mail", true);
    assert!(plan.tool_always_confirm("send-mail"));
}

#[test]
fn tool_always_confirm_false_round_trips() {
    let plan = plugin_with_confirm("read-only", false);
    assert!(!plan.tool_always_confirm("read-only"));
}
