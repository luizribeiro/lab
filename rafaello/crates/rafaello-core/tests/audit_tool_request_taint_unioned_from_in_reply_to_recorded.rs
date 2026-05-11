//! Scope §AL3 / c12 — when the referenced-union arm contributes entries
//! not already covered by `provider-identity ∪ value_match`, exactly one
//! `tool_request_taint_unioned_from_in_reply_to` audit row is written
//! through the broker-installed `AuditWriter`. The payload carries
//! `request_id`, `unioned_entries`, and `in_reply_to_ids`.

#![cfg(feature = "test-fixture")]

use std::sync::Arc;
use std::time::Duration;

use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::reemit::referenced_taint_index::ReferencedTaintIndex;
use rafaello_core::reemit::taint_match::TaintMatchMap;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{
    await_topic, build_rig, subscribe_core_test_receiver, AuditRig, RigOpts, MOCK_PROVIDER_ID,
    READFILE_CANONICAL,
};

#[tokio::test]
async fn audit_tool_request_taint_unioned_from_in_reply_to_recorded() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        tool_routes: vec![("readfile", READFILE_CANONICAL)],
        ..Default::default()
    });
    let audit_rig = AuditRig::new(&rig.broker);
    rig.broker.set_audit_writer(audit_rig.writer.clone());

    let prior_result = JsonRpcId::from("prior-1");
    rig.broker
        .seed_provider_observed_result_for_test(&rig.provider_canonical, prior_result.clone());

    let user_entry = TaintEntry {
        source: "user".to_string(),
        detail: None,
    };
    let taint_match = Arc::new(TaintMatchMap::new(Duration::from_secs(300), 16));
    let referenced = Arc::new(ReferencedTaintIndex::new(Duration::from_secs(300)));
    referenced.record_result(&prior_result, std::slice::from_ref(&user_entry));

    let (mut canonical_rx, _csub) = subscribe_core_test_receiver(&rig.broker);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let router = ReemitRouter::new(
        rig.broker.clone(),
        rig.acl.clone(),
        rig.provider_canonical.clone(),
        shutdown_rx,
    )
    .with_taint_match_map(taint_match.clone())
    .with_referenced_taint_index(referenced.clone());
    let join = router.start();

    let request_id = JsonRpcId::from("req-1");
    let params = serde_json::json!({
        "topic": format!("provider.{MOCK_PROVIDER_ID}.tool_request"),
        "payload": {"tool": "readfile", "args": {"path": "src/main.rs"}},
        "in_reply_to": [prior_result.clone()],
        "request_id": request_id.clone(),
    });
    rig.broker
        .handle_provider_publish(&rig.provider_canonical, &params)
        .expect("publish accepted");

    let mut seen = Vec::new();
    let _ = await_topic(&mut canonical_rx, "core.session.tool_request", &mut seen).await;

    let unioned_rows: Vec<_> = audit_rig
        .rows()
        .into_iter()
        .filter(|r| r.1 == "tool_request_taint_unioned_from_in_reply_to")
        .collect();
    assert_eq!(
        unioned_rows.len(),
        1,
        "expected exactly one §AL3 audit row: {unioned_rows:?}",
    );
    let (_seq, _kind, rid, payload) = &unioned_rows[0];
    assert_eq!(rid.as_deref(), Some(request_id.to_string().as_str()));
    assert_eq!(
        payload.get("request_id"),
        Some(&serde_json::to_value(&request_id).unwrap())
    );
    let entries = payload
        .get("unioned_entries")
        .expect("unioned_entries field")
        .as_array()
        .expect("array");
    assert_eq!(entries.len(), 1);
    let entry: TaintEntry = serde_json::from_value(entries[0].clone()).expect("entry parses");
    assert_eq!(entry, user_entry);
    let ids = payload
        .get("in_reply_to_ids")
        .expect("in_reply_to_ids field")
        .as_array()
        .expect("array");
    assert_eq!(ids.len(), 1);
    let cited: JsonRpcId = serde_json::from_value(ids[0].clone()).expect("id parses");
    assert_eq!(cited, prior_result);

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
