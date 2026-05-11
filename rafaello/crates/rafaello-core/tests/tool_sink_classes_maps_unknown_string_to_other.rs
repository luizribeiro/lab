//! c09 — §Si2. `sinks = ["exec"]` →
//! `vec![SinkClass::Other("exec".into())]`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::compile::{
    CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan, ToolMeta,
};
use rafaello_core::lock::CanonicalId;
use rafaello_core::sinks::SinkClass;

#[test]
fn unknown_sink_string_maps_to_other_variant() {
    let mut tool_meta = BTreeMap::new();
    tool_meta.insert(
        "shell".to_owned(),
        ToolMeta {
            sinks: vec!["exec".to_owned()],
            sinks_inferred: false,
            grant_match: None,
            always_confirm: false,
        },
    );
    let plan = CompiledPlugin {
        canonical: CanonicalId::parse("github.com/acme:shell@1.0.0").unwrap(),
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
    };

    assert_eq!(
        plan.tool_sink_classes("shell"),
        vec![SinkClass::Other("exec".to_owned())]
    );
}
