//! pi-1 H-1 + pi-2 H-1: drives the §CR7 failure path through the
//! **real** `ReemitRouter` body via the
//! `with_test_fault_injector` seam (cfg-gated by `test-fixture`).
//! The injector returns `BrokerError::InvalidTaint { ... }` on a
//! `provider.mock.tool_request`; the router consults the injector,
//! takes the failure path, emits `core.lifecycle.reemit_rejected`,
//! and does NOT fan out a canonical `core.session.tool_request`.

#![cfg(feature = "test-fixture")]

use std::sync::Arc;
use std::time::Duration;

use rafaello_core::bus::JsonRpcId;
use rafaello_core::error::{Publisher, TaintReason};
use rafaello_core::reemit::ReemitRouter;
use rafaello_core::BrokerError;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{
    await_topic, build_rig, drain_for, subscribe_core_test_receiver, RigOpts, MOCK_PROVIDER_ID,
    READFILE_CANONICAL,
};

#[tokio::test]
async fn invalid_taint_emits_reemit_rejected_event() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        tool_routes: vec![("readfile", READFILE_CANONICAL)],
        ..Default::default()
    });

    let prior_result = JsonRpcId::from("tr-fault");
    rig.broker
        .seed_provider_observed_result_for_test(&rig.provider_canonical, prior_result.clone());

    let (mut canonical_rx, _csub) = subscribe_core_test_receiver(&rig.broker);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let router = ReemitRouter::new(
        rig.broker.clone(),
        rig.acl.clone(),
        rig.provider_canonical.clone(),
        shutdown_rx,
    )
    .with_test_fault_injector(Arc::new(|event| {
        if event.topic == "provider.mock.tool_request" {
            Some(BrokerError::InvalidTaint {
                publisher: Publisher::Core,
                topic: "core.session.tool_request".into(),
                reason: TaintReason::Missing,
            })
        } else {
            None
        }
    }));
    let join = router.start();

    let params = serde_json::json!({
        "topic": format!("provider.{MOCK_PROVIDER_ID}.tool_request"),
        "payload": {"tool": "readfile", "args": {"path": "x"}},
        "in_reply_to": [prior_result],
        "request_id": JsonRpcId::from("req-fault"),
    });
    rig.broker
        .handle_provider_publish(&rig.provider_canonical, &params)
        .expect("publish accepted by broker");

    let mut seen = Vec::new();
    let rejected = await_topic(
        &mut canonical_rx,
        "core.lifecycle.reemit_rejected",
        &mut seen,
    )
    .await;
    assert_eq!(
        rejected.payload["inbound_topic"], "provider.mock.tool_request",
        "reemit_rejected names the inbound topic"
    );
    let reason = rejected.payload["reason"]
        .as_str()
        .expect("reason is a string");
    assert!(
        reason.contains("invalid taint") || reason.contains("Missing"),
        "reason carries the structured error: {reason}"
    );

    for ev in &seen {
        assert_ne!(
            ev.topic, "core.session.tool_request",
            "canonical fan-out must not fire on §CR7 failure path"
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
