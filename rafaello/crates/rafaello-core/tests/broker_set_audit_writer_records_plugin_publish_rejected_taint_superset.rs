//! c14 / §PT1 — when the audit writer is installed, a superset
//! violation writes a `plugin_publish_rejected_taint_superset` row.
//! When no writer is installed (initial state) the audit call is
//! silently dropped — the violation is still rejected, but no row is
//! written.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;

use rafaello_core::audit::AuditWriter;
use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId, TaintEntry};
use rafaello_core::lock::CanonicalId;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

fn build_broker() -> (Broker, CanonicalId, String) {
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
    (broker, canonical, topic)
}

fn drive_violation(broker: &Broker, canonical: &CanonicalId, topic: &str) {
    let id = JsonRpcId::from("req-c14h");
    broker
        .publish_for_tool_dispatch(
            canonical,
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
    let (peer, _rx) = fresh_peer();
    let _guard = broker
        .register_plugin(canonical.clone(), peer)
        .expect("registered");
    let params = serde_json::json!({
        "topic": topic,
        "payload": {"ok": true, "content": ""},
        "in_reply_to": [id.clone()],
        "request_id": JsonRpcId::from("resp-c14h"),
        "taint": [{"source": "user", "detail": null}],
    });
    let err = broker
        .handle_plugin_publish(canonical, &params)
        .expect_err("must reject");
    assert!(matches!(err, BrokerError::TaintSupersetViolated { .. }));
}

#[test]
fn audit_row_written_when_writer_installed() {
    let (broker, canonical, topic) = build_broker();
    let tmp = tempfile::tempdir().expect("tmp");
    let writer = AuditWriter::open_for_install(tmp.path()).expect("audit writer");
    broker.set_audit_writer(writer);

    drive_violation(&broker, &canonical, &topic);

    let conn = rusqlite::Connection::open(
        tmp.path()
            .join(".rafaello")
            .join("state")
            .join("session.sqlite"),
    )
    .expect("db");
    let kind: String = conn
        .query_row(
            "SELECT kind FROM audit_events WHERE kind = ?1",
            ["plugin_publish_rejected_taint_superset"],
            |row| row.get(0),
        )
        .expect("audit row exists");
    assert_eq!(kind, "plugin_publish_rejected_taint_superset");
}

#[test]
fn audit_call_silently_dropped_when_writer_unset() {
    let (broker, canonical, topic) = build_broker();
    // Deliberately do NOT call set_audit_writer.
    drive_violation(&broker, &canonical, &topic);
    // No panic, no audit observable. Behaviour signal: the broker
    // returns `None` from `audit_writer()`.
    assert!(broker.audit_writer().is_none());
}
