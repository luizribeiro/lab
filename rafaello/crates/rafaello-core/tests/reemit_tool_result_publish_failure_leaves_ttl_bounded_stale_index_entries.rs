//! pi-5 B-1 (c10 half): on publish failure the handler's
//! `taint_match.record(...)` is already done, so the map entry remains
//! present afterwards. The entry is TTL-bounded stale — once the
//! configured TTL elapses it is dropped on the next `lookup`. Rationale:
//! provenance overreach is harmless; provenance underreach silently
//! drops events. Failure is injected via c08's
//! `Broker::install_publish_test_hook` (NOT the upstream re-emit fault
//! injector) so the failure surfaces *inside* `publish_core_with_taint`,
//! after the handler's `record` call. c13 extends this test to cover
//! the ancestry-union arm.

#![cfg(feature = "test-fixture")]

use std::sync::Arc;
use std::time::Duration;

use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::error::BrokerError;
use rafaello_core::reemit::taint_match::TaintMatchMap;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{
    await_topic, build_rig, subscribe_core_test_receiver, RigOpts, READFILE_CANONICAL,
    READFILE_TOPIC_ID,
};

#[tokio::test(start_paused = true)]
async fn reemit_tool_result_publish_failure_leaves_ttl_bounded_stale_index_entries() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        ..Default::default()
    });
    let plugin_canonical = rig.readfile_canonical.clone().expect("readfile present");

    let ttl = Duration::from_secs(60);
    let shared = Arc::new(TaintMatchMap::new(ttl, 16));
    let verbatim = "verbatim string above sixteen bytes";

    rig.broker.install_publish_test_hook(Arc::new(move |event| {
        if event.topic == "core.session.tool_result" {
            Some(BrokerError::Internal {
                detail: "c10 induced publish failure".to_string(),
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
    .with_taint_match_map(shared.clone());
    let join = router.start();

    let tool_request_id = JsonRpcId::from("tool-req-1");
    let request_id = JsonRpcId::from("res-1");
    let inbound_payload = serde_json::json!({"content": verbatim});
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
        "payload": inbound_payload,
        "in_reply_to": [tool_request_id],
        "request_id": request_id,
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

    let hits = shared.lookup(&serde_json::json!({"arg": verbatim}));
    let expected = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some(READFILE_CANONICAL.to_string()),
    }];
    assert_eq!(
        hits, expected,
        "record runs before publish; failed publish leaves the entry in the map",
    );

    tokio::time::advance(ttl + Duration::from_secs(1)).await;
    let after_ttl = shared.lookup(&serde_json::json!({"arg": verbatim}));
    assert!(
        after_ttl.is_empty(),
        "TTL-bounded stale: entry must be dropped after TTL elapses, got {after_ttl:?}",
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
