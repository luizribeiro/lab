//! c14 / §PT1 — a plugin publishing `tool_result` with `taint` that is
//! not a superset of the dispatch entry's canonical
//! `tool_request_taint` is rejected with
//! `BrokerError::TaintSupersetViolated`, an audit row is written, a
//! `core.lifecycle.publish_rejected` event is published (code
//! `taint_superset_violated`), and a synthetic deny-shaped
//! `core.session.tool_result` is observable on an internal subscriber.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;

use rafaello_core::audit::AuditWriter;
use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId, TaintEntry};
use rafaello_core::lock::CanonicalId;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn plugin_tool_result_taint_superset_violation_rejected() {
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
    let tmp = tempfile::tempdir().expect("state tempdir");
    let writer = AuditWriter::open_for_install(tmp.path()).expect("audit writer opens");
    broker.set_audit_writer(writer);

    let (peer, _rx) = fresh_peer();
    let _guard = broker
        .register_plugin(canonical.clone(), peer)
        .expect("registered");

    let (mut lifecycle_rx, _lifecycle_sub) =
        broker.subscribe_internal(vec!["core.lifecycle.publish_rejected".to_string()], 16);
    let (mut synthetic_rx, _synthetic_sub) =
        broker.subscribe_internal(vec!["core.session.tool_result".to_string()], 16);

    let dispatch_id = JsonRpcId::from("req-c14a");
    let dispatch_taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some("rafaello-fetch".to_string()),
    }];
    broker
        .publish_for_tool_dispatch(
            &canonical,
            serde_json::json!({}),
            dispatch_id.clone(),
            None,
            None,
            dispatch_taint.clone(),
        )
        .expect("dispatch ok");

    let bad_taint = vec![TaintEntry {
        source: "plugin".to_string(),
        detail: Some("local/test:other@0.1.0".to_string()),
    }];
    let params = serde_json::json!({
        "topic": topic,
        "payload": {"ok": true, "content": "leaked"},
        "in_reply_to": [dispatch_id.clone()],
        "request_id": JsonRpcId::from("resp-c14a"),
        "taint": bad_taint,
    });

    let err = broker
        .handle_plugin_publish(&canonical, &params)
        .expect_err("superset violation must be rejected");
    match err {
        BrokerError::TaintSupersetViolated {
            topic: ref t,
            ref missing,
            ..
        } => {
            assert!(t.ends_with(".tool_result"), "topic={t}");
            assert_eq!(
                missing, &dispatch_taint,
                "missing should equal dispatch taint not in published"
            );
        }
        other => panic!("expected TaintSupersetViolated, got {other:?}"),
    }

    let synthetic = synthetic_rx
        .try_recv()
        .expect("synthetic tool_result observable");
    assert_eq!(synthetic.topic, "core.session.tool_result");
    assert_eq!(synthetic.payload["ok"], false);
    assert_eq!(
        synthetic.payload["error"],
        "plugin_taint_superset_violation"
    );
    assert_eq!(synthetic.payload["content"], "");
    assert_eq!(
        synthetic.in_reply_to.as_deref(),
        Some(&[dispatch_id.clone()][..]),
        "synthetic in_reply_to cites originating tool_request id"
    );
    assert_eq!(
        synthetic.taint.as_deref(),
        Some(&dispatch_taint[..]),
        "synthetic taint preserves canonical ancestry"
    );

    let lifecycle = lifecycle_rx
        .try_recv()
        .expect("lifecycle publish_rejected observable");
    assert_eq!(lifecycle.topic, "core.lifecycle.publish_rejected");
    assert_eq!(lifecycle.payload["code"], "taint_superset_violated");

    let conn = rusqlite::Connection::open(
        tmp.path()
            .join(".rafaello")
            .join("state")
            .join("session.sqlite"),
    )
    .expect("audit db");
    let kind: String = conn
        .query_row(
            "SELECT kind FROM audit_events ORDER BY seq DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("audit row present");
    assert_eq!(kind, "plugin_publish_rejected_taint_superset");
}
