//! pi-1 H-1: a provider publishes `provider.mock.tool_request` for
//! a tool name absent from `acl.tool_routes`. The router observes
//! the inbound event, fails the `tool_routes` lookup, emits
//! `core.lifecycle.tool_dispatch_rejected` with `reason: "unknown_tool"`,
//! and does NOT publish a canonical `core.session.tool_request`.

#![cfg(feature = "test-fixture")]

use std::time::Duration;

use rafaello_core::bus::JsonRpcId;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{
    await_topic, build_rig, drain_for, subscribe_core_test_receiver, RigOpts, MOCK_PROVIDER_ID,
};

#[tokio::test]
async fn unknown_tool_emits_tool_dispatch_rejected_event() {
    // No tool_routes entries — every tool name is unknown.
    let rig = build_rig(RigOpts::default());

    let prior_result = JsonRpcId::from("tr-unknown");
    rig.broker
        .seed_provider_observed_result_for_test(&rig.provider_canonical, prior_result.clone());

    let (mut canonical_rx, _csub) = subscribe_core_test_receiver(&rig.broker);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let router = ReemitRouter::new(
        rig.broker.clone(),
        rig.acl.clone(),
        rig.provider_canonical.clone(),
        shutdown_rx,
    );
    let join = router.start();

    let params = serde_json::json!({
        "topic": format!("provider.{MOCK_PROVIDER_ID}.tool_request"),
        "payload": {"tool": "nonexistent", "args": {}},
        "in_reply_to": [prior_result],
        "request_id": JsonRpcId::from("req-unknown"),
    });
    rig.broker
        .handle_provider_publish(&rig.provider_canonical, &params)
        .expect("broker accepts inbound publish");

    let mut seen = Vec::new();
    let rejected = await_topic(
        &mut canonical_rx,
        "core.lifecycle.tool_dispatch_rejected",
        &mut seen,
    )
    .await;
    assert_eq!(rejected.payload["tool"], "nonexistent");
    assert_eq!(rejected.payload["reason"], "unknown_tool");

    for ev in &seen {
        assert_ne!(
            ev.topic, "core.session.tool_request",
            "canonical re-emit must not fire when tool route is missing"
        );
    }
    let tail = drain_for(&mut canonical_rx, Duration::from_millis(100)).await;
    for ev in &tail {
        assert_ne!(ev.topic, "core.session.tool_request");
    }

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
