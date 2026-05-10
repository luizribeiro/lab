//! Path A — entry kind not in registry, no author fallback → emit warn
//! Callout containing a KeyValue with kind/schema/payload, per Stream E §6
//! second bullet.

use std::sync::Arc;

use rafaello_core::entry::render_node::CalloutKind;
use rafaello_core::entry::{Entry, RenderNode};
use rafaello_core::{Capabilities, RenderPipeline, RendererRegistry};
use serde_json::json;

#[test]
fn unknown_kind_no_fallback_emits_warn_callout_keyvalue() {
    let mut entry = Entry::new_text("ignored");
    entry.kind = "synthetic:no-such-renderer".into();
    entry.schema = Some("v1".into());
    entry.payload = json!({ "alpha": 1 });
    entry.fallback = None;

    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::new()));
    let out = pipeline.render(&entry, &Capabilities::tui_default());

    let RenderNode::Callout { kind, child } = out else {
        panic!("expected Callout");
    };
    assert!(matches!(kind, CalloutKind::Warn));

    let RenderNode::KeyValue { pairs } = *child else {
        panic!("expected KeyValue child");
    };
    assert_eq!(pairs.len(), 3);

    let by_key: std::collections::HashMap<_, _> = pairs
        .iter()
        .map(|p| {
            let v = match &p.value {
                RenderNode::Text { text, .. } => text.clone(),
                other => panic!("expected Text value, got {other:?}"),
            };
            (p.key.clone(), v)
        })
        .collect();
    assert_eq!(by_key["kind"], "synthetic:no-such-renderer");
    assert_eq!(by_key["schema"], "v1");
    assert!(by_key["payload"].contains("\"alpha\""));
    assert!(by_key["payload"].contains("1"));
}

#[test]
fn unknown_kind_no_fallback_no_schema_yields_empty_schema_string() {
    let mut entry = Entry::new_text("ignored");
    entry.kind = "synthetic:other".into();
    entry.schema = None;
    entry.payload = json!({});
    entry.fallback = None;

    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::new()));
    let out = pipeline.render(&entry, &Capabilities::tui_default());

    let RenderNode::Callout { child, .. } = out else {
        panic!("expected Callout");
    };
    let RenderNode::KeyValue { pairs } = *child else {
        panic!("expected KeyValue child");
    };
    let schema = pairs
        .iter()
        .find(|p| p.key == "schema")
        .expect("schema key");
    match &schema.value {
        RenderNode::Text { text, .. } => assert_eq!(text, ""),
        other => panic!("expected Text, got {other:?}"),
    }
}
