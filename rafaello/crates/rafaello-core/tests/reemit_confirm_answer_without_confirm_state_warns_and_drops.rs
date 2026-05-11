//! c14 / pi-2 M-5 transitional contract: a `ReemitRouter` constructed
//! without `.with_confirm_state_and_audit(...)` (m4-shaped call site)
//! receives a well-formed `frontend.tui.confirm_answer` and **drops**
//! it with a `tracing::warn!`. No `core.session.confirm_reply` is
//! emitted; the other re-emit arms (`user_message`, `tool_request`,
//! `assistant_message`, `tool_result`) continue to re-emit normally.

use std::time::Duration;

use rafaello_core::bus::JsonRpcId;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{
    await_topic, build_rig, drain_for, subscribe_core_test_receiver, RigOpts, TUI_ATTACH_ID,
};

#[tokio::test]
#[tracing_test::traced_test]
async fn confirm_answer_without_confirm_state_warns_and_drops() {
    let rig = build_rig(RigOpts {
        include_tui_frontend: true,
        tui_publish_confirm_answer: true,
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

    let correlation = JsonRpcId::from(ulid::Ulid::new().to_string());
    let params = serde_json::json!({
        "topic": format!("frontend.{TUI_ATTACH_ID}.confirm_answer"),
        "payload": {"request_id": correlation.to_string(), "answer": "allow"},
        "in_reply_to": [correlation.clone()],
        "request_id": JsonRpcId::from(ulid::Ulid::new().to_string()),
    });
    rig.broker
        .handle_frontend_publish(&attach, &params)
        .expect("publish accepted");

    let observed = drain_for(&mut canonical_rx, Duration::from_millis(200)).await;
    for ev in &observed {
        assert_ne!(
            ev.topic, "core.session.confirm_reply",
            "no confirm_reply emitted when confirm_state is not wired"
        );
        assert_ne!(
            ev.topic, "core.lifecycle.reemit_rejected",
            "transitional drop is not a §CR7 failure"
        );
    }
    assert!(
        logs_contain("confirm_state-not-wired"),
        "expected a `confirm_state-not-wired` tracing::warn!"
    );

    // m4-shaped arm: user_message still re-emits normally.
    let user_id = JsonRpcId::from(ulid::Ulid::new().to_string());
    let user_params = serde_json::json!({
        "topic": format!("frontend.{TUI_ATTACH_ID}.user_message"),
        "payload": {"text": "hi"},
        "request_id": user_id.clone(),
    });
    rig.broker
        .handle_frontend_publish(&attach, &user_params)
        .expect("user_message publish accepted");
    let mut seen = Vec::new();
    let user_event = await_topic(&mut canonical_rx, "core.session.user_message", &mut seen).await;
    assert_eq!(user_event.request_id, Some(user_id));

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
