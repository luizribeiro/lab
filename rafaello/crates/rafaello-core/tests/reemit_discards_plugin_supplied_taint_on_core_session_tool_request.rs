//! Round-1/round-2 negative: a provider publishes
//! `provider.mock.tool_request` with an inbound `taint:
//! [{source: "user"}]`. The broker discards it at the inbound side
//! (§B6 step 8) and the router synthesises the canonical provider
//! taint anyway — the canonical `core.session.tool_request` carries
//! `[{source: "provider", detail: "mock"}]` only.

#![cfg(feature = "test-fixture")]

use std::time::Duration;

use rafaello_core::bus::JsonRpcId;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{
    assert_origin_taint, await_topic, build_rig, subscribe_core_test_receiver, RigOpts,
    MOCK_PROVIDER_ID, READFILE_CANONICAL,
};

#[tokio::test]
async fn provider_tool_request_discards_inbound_taint() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        tool_routes: vec![("readfile", READFILE_CANONICAL)],
        ..Default::default()
    });

    let prior_result = JsonRpcId::from("tr-spoof");
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
        "payload": {"tool": "readfile", "args": {"path": "x"}},
        "in_reply_to": [prior_result],
        "request_id": JsonRpcId::from("req-spoof"),
        "taint": [{"source": "user", "detail": null}],
    });
    rig.broker
        .handle_provider_publish(&rig.provider_canonical, &params)
        .expect("publish accepted");

    let mut seen = Vec::new();
    let event = await_topic(&mut canonical_rx, "core.session.tool_request", &mut seen).await;
    assert_origin_taint(&event, "provider", Some(MOCK_PROVIDER_ID));

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
