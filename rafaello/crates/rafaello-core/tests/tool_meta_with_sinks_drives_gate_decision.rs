//! c09 — §Si2/§Si3. A `CompiledPlugin` with
//! `tool_meta["send-mail"].sinks = ["mail"]` returns
//! `vec![SinkClass::Mail]` from `tool_sink_classes`, and `tool_sinks`
//! returns the underlying string slice `&["mail".to_string()]`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::compile::{
    CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan, ToolMeta,
};
use rafaello_core::lock::CanonicalId;
use rafaello_core::sinks::SinkClass;

fn plugin_with(tool: &str, sinks: Vec<String>, always_confirm: bool) -> CompiledPlugin {
    let mut tool_meta = BTreeMap::new();
    tool_meta.insert(
        tool.to_owned(),
        ToolMeta {
            sinks,
            sinks_inferred: false,
            grant_match: None,
            always_confirm,
        },
    );
    CompiledPlugin {
        canonical: CanonicalId::parse("github.com/acme:mailer@1.0.0").unwrap(),
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
fn tool_sinks_returns_underlying_string_slice() {
    let plan = plugin_with("send-mail", vec!["mail".to_owned()], false);
    assert_eq!(plan.tool_sinks("send-mail"), Some(&["mail".to_owned()][..]));
}

#[test]
fn tool_sink_classes_maps_mail_string_to_mail_variant() {
    let plan = plugin_with("send-mail", vec!["mail".to_owned()], false);
    assert_eq!(plan.tool_sink_classes("send-mail"), vec![SinkClass::Mail]);
}
