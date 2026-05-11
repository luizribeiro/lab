//! c14 / §PT1 — after a superset-violation rejection, the outstanding
//! dispatch entry is drained: a duplicate publish (same id) from the
//! same plugin no longer finds the entry and is rejected with
//! `StaleRequestId` (no retry window).

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId, TaintEntry};
use rafaello_core::lock::CanonicalId;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn superset_violation_drains_outstanding_entry() {
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

    let id = JsonRpcId::from("req-c14e");
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
    assert_eq!(broker.outstanding_dispatched_count(&canonical), 1);

    let bad_taint = vec![TaintEntry {
        source: "user".to_string(),
        detail: None,
    }];
    let params = serde_json::json!({
        "topic": topic,
        "payload": {"ok": true, "content": "x"},
        "in_reply_to": [id.clone()],
        "request_id": JsonRpcId::from("resp-c14e"),
        "taint": bad_taint,
    });
    let err = broker
        .handle_plugin_publish(&canonical, &params)
        .expect_err("violation must be rejected");
    assert!(matches!(err, BrokerError::TaintSupersetViolated { .. }));
    assert_eq!(
        broker.outstanding_dispatched_count(&canonical),
        0,
        "outstanding entry drained on violation"
    );
    assert!(
        broker.peek_outstanding_for_test(&canonical, &id).is_none(),
        "entry must not be re-discoverable"
    );

    // Replay → no entry → StaleRequestId (no retry window).
    let err2 = broker
        .handle_plugin_publish(&canonical, &params)
        .expect_err("replay must be StaleRequestId");
    assert!(
        matches!(err2, BrokerError::StaleRequestId { id: ref i, .. } if i == &id),
        "expected StaleRequestId on replay, got {err2:?}"
    );
}
