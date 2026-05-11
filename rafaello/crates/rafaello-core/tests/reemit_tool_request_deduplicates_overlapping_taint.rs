//! Scope §TR3 step 4 / c12 — when the same `TaintEntry` is contributed
//! by more than one arm (here: the value-match arm and the referenced-
//! union arm both yield the provider-identity entry), the canonical
//! taint deduplicates it.

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
async fn reemit_tool_request_deduplicates_overlapping_taint() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        tool_routes: vec![("readfile", READFILE_CANONICAL)],
        ..Default::default()
    });

    let prior_result = JsonRpcId::from("prior-1");
    rig.broker
        .seed_provider_observed_result_for_test(&rig.provider_canonical, prior_result.clone());

    let provider_entry = TaintEntry {
        source: "provider".to_string(),
        detail: Some(MOCK_PROVIDER_ID.to_string()),
    };

    let taint_match = Arc::new(TaintMatchMap::new(Duration::from_secs(300), 16));
    taint_match.record(
        &serde_json::json!("dupe-token-thats-long-enough-32"),
        std::slice::from_ref(&provider_entry),
    );

    let referenced = Arc::new(ReferencedTaintIndex::new(Duration::from_secs(300)));
    referenced.record_result(&prior_result, std::slice::from_ref(&provider_entry));

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
        "payload": {"tool": "readfile", "args": {"q": "dupe-token-thats-long-enough-32"}},
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
    assert_eq!(
        taint,
        &vec![provider_entry],
        "overlapping arms must dedup to a single provider entry: {taint:?}",
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
