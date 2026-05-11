//! pi-5 B-1 (c11 half): on publish failure the handler's
//! `referenced_taint_index.record_request(...)` is already done, so the
//! `by_request_id` entry remains present afterwards. The entry is
//! TTL-bounded stale — once the configured TTL elapses it is dropped on
//! the next `lookup`. A misbehaving plugin fabricating the id is
//! rejected by the m5a broker stale-id check. Failure is injected via
//! c08's `Broker::install_publish_test_hook` so the failure surfaces
//! *inside* `publish_core_with_taint`, after the handler's
//! `record_request` call.

#![cfg(feature = "test-fixture")]

use std::sync::Arc;
use std::time::Duration;

use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::error::BrokerError;
use rafaello_core::reemit::referenced_taint_index::ReferencedTaintIndex;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{
    await_topic, build_rig, subscribe_core_test_receiver, RigOpts, MOCK_PROVIDER_ID,
    READFILE_CANONICAL,
};

#[tokio::test(start_paused = true)]
async fn reemit_tool_request_publish_failure_leaves_ttl_bounded_stale_request_entry() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        tool_routes: vec![("readfile", READFILE_CANONICAL)],
        ..Default::default()
    });

    let prior_result = JsonRpcId::from("prior-1");
    rig.broker
        .seed_provider_observed_result_for_test(&rig.provider_canonical, prior_result.clone());

    let ttl = Duration::from_secs(60);
    let shared = Arc::new(ReferencedTaintIndex::new(ttl));

    rig.broker.install_publish_test_hook(Arc::new(move |event| {
        if event.topic == "core.session.tool_request" {
            Some(BrokerError::Internal {
                detail: "c11 induced publish failure".to_string(),
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
    .with_referenced_taint_index(shared.clone());
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
        Some(format!("provider.{MOCK_PROVIDER_ID}.tool_request").as_str()),
    );

    let hit = shared.lookup_request(&request_id);
    let expected = vec![TaintEntry {
        source: "provider".to_string(),
        detail: Some(MOCK_PROVIDER_ID.to_string()),
    }];
    assert_eq!(
        hit,
        Some(expected),
        "record_request runs before publish; failed publish leaves the entry in by_request_id",
    );

    tokio::time::advance(ttl + Duration::from_secs(1)).await;
    let after_ttl = shared.lookup_request(&request_id);
    assert!(
        after_ttl.is_none(),
        "TTL-bounded stale: entry must be dropped after TTL elapses, got {after_ttl:?}",
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
