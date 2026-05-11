//! CR3 happy path: a tool plugin publishes
//! `plugin.<topic-id>.tool_result`; the `ReemitRouter` emits
//! `core.session.tool_result` with payload forwarded byte-for-byte,
//! canonical tool taint `[{source: "tool", detail: "<canonical>"}]`,
//! and `in_reply_to` / `request_id` forwarded verbatim.

use std::time::Duration;

use rafaello_core::bus::JsonRpcId;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{
    assert_origin_taint, await_topic, build_rig, subscribe_core_test_receiver, RigOpts,
    READFILE_CANONICAL, READFILE_TOPIC_ID,
};

#[tokio::test]
async fn plugin_tool_result_reemitted_as_core_session_tool_result() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        ..Default::default()
    });
    let plugin_canonical = rig.readfile_canonical.clone().expect("readfile present");

    let (mut canonical_rx, _csub) = subscribe_core_test_receiver(&rig.broker);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let router = ReemitRouter::new(
        rig.broker.clone(),
        rig.acl.clone(),
        rig.provider_canonical.clone(),
        shutdown_rx,
    );
    let join = router.start();

    let tool_request_id = JsonRpcId::from("tool-req-1");
    let request_id = JsonRpcId::from("res-1");
    let inbound_payload = serde_json::json!({"ok": true, "content": "fn main() {}"});
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
        "payload": inbound_payload.clone(),
        "in_reply_to": [tool_request_id.clone()],
        "request_id": request_id.clone(),
    });
    rig.broker
        .handle_plugin_publish(&plugin_canonical, &params)
        .expect("publish accepted");

    let mut seen = Vec::new();
    let event = await_topic(&mut canonical_rx, "core.session.tool_result", &mut seen).await;
    assert_origin_taint(&event, "tool", Some(READFILE_CANONICAL));
    assert_eq!(event.payload, inbound_payload, "payload byte-equal");
    assert_eq!(event.request_id, Some(request_id));
    assert_eq!(
        event.in_reply_to.as_deref(),
        Some(&[tool_request_id][..]),
        "in_reply_to forwarded"
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
