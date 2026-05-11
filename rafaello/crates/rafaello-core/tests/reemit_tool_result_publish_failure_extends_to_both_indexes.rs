//! Scope §TR1 publish-failure stale (c13, pi-4 N-1; replaces c10's
//! single-arm test): on `publish_core_with_taint` failure, both
//! `TaintMatchMap` and `ReferencedTaintIndex.by_result_id` already
//! carry the recorded unioned entry. Both are TTL-bounded stale —
//! after the configured TTL elapses they disappear. Failure is
//! injected via c08's `Broker::install_publish_test_hook` so it
//! surfaces *inside* `publish_core_with_taint`, after the handler's
//! `record` calls.

#![cfg(feature = "test-fixture")]

use std::sync::Arc;
use std::time::Duration;

use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::error::BrokerError;
use rafaello_core::reemit::referenced_taint_index::ReferencedTaintIndex;
use rafaello_core::reemit::taint_match::TaintMatchMap;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{
    await_topic, build_rig, subscribe_core_test_receiver, RigOpts, READFILE_CANONICAL,
    READFILE_TOPIC_ID,
};

#[tokio::test(start_paused = true)]
async fn reemit_tool_result_publish_failure_extends_to_both_indexes() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        ..Default::default()
    });
    let plugin_canonical = rig.readfile_canonical.clone().expect("readfile present");

    let ttl = Duration::from_secs(60);
    let taint_match = Arc::new(TaintMatchMap::new(ttl, 16));
    let referenced = Arc::new(ReferencedTaintIndex::new(ttl));

    let tool_request_id = JsonRpcId::from("tool-req-1");
    let result_id = JsonRpcId::from("res-1");
    let prior_user_taint = vec![TaintEntry {
        source: "user".to_string(),
        detail: None,
    }];
    referenced.record_request(&tool_request_id, &prior_user_taint);

    let verbatim = "verbatim string above sixteen bytes";

    rig.broker.install_publish_test_hook(Arc::new(move |event| {
        if event.topic == "core.session.tool_result" {
            Some(BrokerError::Internal {
                detail: "c13 induced publish failure".to_string(),
            })
        } else {
            None
        }
    }));

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

    rig.broker
        .publish_for_tool_dispatch(
            &plugin_canonical,
            serde_json::json!({}),
            tool_request_id.clone(),
            None,
            None,
            Vec::new(),
        )
        .expect("dispatch seeds outstanding map");
    let params = serde_json::json!({
        "topic": format!("plugin.{READFILE_TOPIC_ID}.tool_result"),
        "payload": {"content": verbatim},
        "in_reply_to": [tool_request_id],
        "request_id": result_id.clone(),
    });
    rig.broker
        .handle_plugin_publish(&plugin_canonical, &params)
        .expect("publish accepted");

    let mut seen = Vec::new();
    let rejected = await_topic(
        &mut canonical_rx,
        "core.lifecycle.reemit_rejected",
        &mut seen,
    )
    .await;
    assert_eq!(
        rejected
            .payload
            .get("inbound_topic")
            .and_then(|v| v.as_str()),
        Some(format!("plugin.{READFILE_TOPIC_ID}.tool_result").as_str()),
    );

    let mut expected = vec![
        TaintEntry {
            source: "tool".to_string(),
            detail: Some(READFILE_CANONICAL.to_string()),
        },
        TaintEntry {
            source: "user".to_string(),
            detail: None,
        },
    ];
    expected.sort_by(|a, b| {
        (a.source.as_str(), a.detail.as_deref()).cmp(&(b.source.as_str(), b.detail.as_deref()))
    });

    let match_hits = taint_match.lookup(&serde_json::json!({"arg": verbatim}));
    assert_eq!(
        match_hits, expected,
        "TaintMatchMap entry persists past publish failure with unioned vector",
    );
    let result_hits = referenced
        .lookup_result(&result_id)
        .expect("by_result_id entry persists past publish failure");
    assert_eq!(
        result_hits, expected,
        "ReferencedTaintIndex.by_result_id entry persists past publish failure",
    );

    tokio::time::advance(ttl + Duration::from_secs(1)).await;
    let match_after = taint_match.lookup(&serde_json::json!({"arg": verbatim}));
    assert!(
        match_after.is_empty(),
        "TaintMatchMap entry must expire after TTL, got {match_after:?}",
    );
    let result_after = referenced.lookup_result(&result_id);
    assert!(
        result_after.is_none(),
        "ReferencedTaintIndex.by_result_id entry must expire after TTL, got {result_after:?}",
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
