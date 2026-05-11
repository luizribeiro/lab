//! Scope §TR3 step 2 / c12 — `handle_tool_request` unions
//! `TaintMatchMap::lookup(args)` into the canonical taint vector. Here
//! the map is pre-seeded with a value-driven `tool`-source taint; when
//! a later tool_request's args contain that value, the canonical taint
//! contains BOTH the provider-identity entry AND the value-match entry.

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
async fn reemit_tool_request_unions_value_driven_taint() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        tool_routes: vec![("readfile", READFILE_CANONICAL)],
        ..Default::default()
    });

    let prior_result = JsonRpcId::from("prior-1");
    rig.broker
        .seed_provider_observed_result_for_test(&rig.provider_canonical, prior_result.clone());

    let taint_match = Arc::new(TaintMatchMap::new(Duration::from_secs(300), 16));
    let recorded_taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some(READFILE_CANONICAL.to_string()),
    }];
    taint_match.record(
        &serde_json::json!({"secret_value_with_enough_length_for_substring": "match-me-please-32bytes"}),
        &recorded_taint,
    );

    let referenced = Arc::new(ReferencedTaintIndex::new(Duration::from_secs(300)));

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
        "payload": {"tool": "readfile", "args": {"q": "match-me-please-32bytes"}},
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
    let tool_entry = TaintEntry {
        source: "tool".to_string(),
        detail: Some(READFILE_CANONICAL.to_string()),
    };
    assert!(
        taint.contains(&provider_entry),
        "canonical taint must contain provider-identity entry: {taint:?}",
    );
    assert!(
        taint.contains(&tool_entry),
        "canonical taint must contain value-match entry: {taint:?}",
    );
    assert_eq!(taint.len(), 2, "exactly provider + value-match: {taint:?}");

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
