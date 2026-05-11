//! c14 / §PT1 / pi-2 M-5 — a plugin `tool_result` with `taint`
//! omitted (`None` after deserialisation) skips the superset check
//! entirely: the publish is accepted even though the dispatch entry
//! carried a non-empty canonical `tool_request_taint`.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId, TaintEntry};
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn plugin_tool_result_with_none_taint_passes_check() {
    let canonical = CanonicalId::parse("local/test:plug@0.1.0").expect("canonical");
    let topic_id = "plug_local_test";
    let topic = format!("plugin.{topic_id}.tool_result");
    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: topic_id.to_string(),
            publish_topics: vec![topic.clone()],
            subscribe_patterns: vec![],
            auto_subscribes: vec![format!("plugin.{topic_id}.tool_request")],
            provider_id: None,
        },
    );
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");
    let (peer, _rx) = fresh_peer();
    let _guard = broker
        .register_plugin(canonical.clone(), peer)
        .expect("registered");

    let id = JsonRpcId::from("req-c14b");
    broker
        .publish_for_tool_dispatch(
            &canonical,
            serde_json::json!({}),
            id.clone(),
            None,
            None,
            vec![TaintEntry {
                source: "tool".to_string(),
                detail: Some("rafaello-fetch".to_string()),
            }],
        )
        .expect("dispatch ok");

    // No `taint` field → deserialises as `None`.
    let params = serde_json::json!({
        "topic": topic,
        "payload": {"ok": true, "content": "ok"},
        "in_reply_to": [id.clone()],
        "request_id": JsonRpcId::from("resp-c14b"),
    });
    broker
        .handle_plugin_publish(&canonical, &params)
        .expect("None taint skips superset check");
}
