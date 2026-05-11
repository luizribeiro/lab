//! CR5 happy path: the TUI frontend publishes
//! `frontend.tui.user_message` with a fresh `request_id`; the
//! `ReemitRouter` emits `core.session.user_message` with canonical
//! user taint `[{source: "user", detail: None}]`, the inbound
//! `request_id` forwarded, and `in_reply_to = None` (root message).

use std::time::Duration;

use rafaello_core::bus::JsonRpcId;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{
    assert_origin_taint, await_topic, build_rig, subscribe_core_test_receiver, RigOpts,
    TUI_ATTACH_ID,
};

#[tokio::test]
async fn frontend_user_message_reemitted_as_core_session_user_message() {
    let rig = build_rig(RigOpts {
        include_tui_frontend: true,
        ..Default::default()
    });
    let attach = rig.frontend_attach.clone().expect("tui registered");

    let (mut canonical_rx, _csub) = subscribe_core_test_receiver(&rig.broker);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let router = ReemitRouter::new(
        rig.broker.clone(),
        rig.acl.clone(),
        rig.provider_canonical.clone(),
        shutdown_rx,
    );
    let join = router.start();

    let request_id = JsonRpcId::from("ulid-1");
    let params = serde_json::json!({
        "topic": format!("frontend.{TUI_ATTACH_ID}.user_message"),
        "payload": {"text": "hello"},
        "request_id": request_id.clone(),
    });
    rig.broker
        .handle_frontend_publish(&attach, &params)
        .expect("publish accepted");

    let mut seen = Vec::new();
    let event = await_topic(&mut canonical_rx, "core.session.user_message", &mut seen).await;
    assert_origin_taint(&event, "user", None);
    assert_eq!(event.payload["text"], "hello");
    assert_eq!(event.request_id, Some(request_id));
    assert!(event.in_reply_to.is_none(), "user messages are roots");

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
