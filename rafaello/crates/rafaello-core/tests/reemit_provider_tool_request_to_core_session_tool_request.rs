//! CR2 happy path: a provider publishes
//! `provider.mock.tool_request` with a valid `tool` field that
//! resolves through `acl.tool_routes`; the `ReemitRouter` emits
//! `core.session.tool_request` carrying canonical provider taint
//! and a `dispatch_target` payload field.

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
async fn provider_tool_request_reemitted_as_core_session_tool_request() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        tool_routes: vec![("readfile", READFILE_CANONICAL)],
        ..Default::default()
    });

    let prior_result = JsonRpcId::from("tr-1");
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
    let event = await_topic(&mut canonical_rx, "core.session.tool_request", &mut seen).await;
    assert_origin_taint(&event, "provider", Some(MOCK_PROVIDER_ID));
    assert_eq!(
        event.payload["dispatch_target"].as_str(),
        Some(READFILE_CANONICAL),
        "dispatch_target points at the tool plugin"
    );
    assert_eq!(event.payload["tool"], "readfile");
    assert_eq!(event.payload["args"]["path"], "src/main.rs");
    assert_eq!(event.request_id, Some(request_id));
    assert_eq!(event.in_reply_to.as_deref(), Some(&[prior_result][..]));

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
