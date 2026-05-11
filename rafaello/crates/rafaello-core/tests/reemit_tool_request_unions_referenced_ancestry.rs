//! Scope §TR3 step 3 / c12 — the referenced-union arm unions the
//! taints of `ReferencedTaintIndex::lookup_result(id)` for each id in
//! `event.in_reply_to` into the canonical taint. Seeded directly via
//! `record_result` so this test is isolated from c13 wiring.

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
    await_topic, build_rig, subscribe_core_test_receiver, RigOpts, MOCK_PROVIDER_ID,
    READFILE_CANONICAL,
};

#[tokio::test]
async fn reemit_tool_request_unions_referenced_ancestry() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        tool_routes: vec![("readfile", READFILE_CANONICAL)],
        ..Default::default()
    });

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
        "in_reply_to": [prior_result],
        "request_id": request_id.clone(),
    });
    rig.broker
        .handle_provider_publish(&rig.provider_canonical, &params)
        .expect("publish accepted");

    let mut seen = Vec::new();
    let canonical = await_topic(&mut canonical_rx, "core.session.tool_request", &mut seen).await;
    let taint = canonical
        .taint
        .as_ref()
        .expect("canonical tool_request carries taint");

    let provider_entry = TaintEntry {
        source: "provider".to_string(),
        detail: Some(MOCK_PROVIDER_ID.to_string()),
    };
    assert!(
        taint.contains(&user_entry),
        "referenced-ancestry user entry must be unioned in: {taint:?}",
    );
    assert!(
        taint.contains(&provider_entry),
        "provider-identity entry must be present: {taint:?}",
    );
    assert_eq!(
        taint.len(),
        2,
        "expected exactly provider + user: {taint:?}"
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
