//! c14 / §PT1 / pi-5 M-2 — `core.lifecycle.publish_rejected` for a
//! superset violation is emitted exactly once, by the **outer**
//! `emit_publish_rejected_for_plugin` wrapper. The inner handler must
//! not publish a duplicate lifecycle event.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId, TaintEntry};
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn pt1_lifecycle_emitted_by_outer_wrapper_only() {
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

    let (mut lifecycle_rx, _sub) =
        broker.subscribe_internal(vec!["core.lifecycle.publish_rejected".to_string()], 16);

    let id = JsonRpcId::from("req-c14g");
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

    let params = serde_json::json!({
        "topic": topic,
        "payload": {"ok": true, "content": ""},
        "in_reply_to": [id.clone()],
        "request_id": JsonRpcId::from("resp-c14g"),
        "taint": [{"source": "user", "detail": null}],
    });

    let _ = broker.handle_plugin_publish(&canonical, &params);

    let first = lifecycle_rx
        .try_recv()
        .expect("one lifecycle event observable");
    assert_eq!(first.topic, "core.lifecycle.publish_rejected");
    assert_eq!(first.payload["code"], "taint_superset_violated");
    assert!(
        lifecycle_rx.try_recv().is_err(),
        "exactly one publish_rejected event — outer wrapper owns lifecycle emission"
    );
}
