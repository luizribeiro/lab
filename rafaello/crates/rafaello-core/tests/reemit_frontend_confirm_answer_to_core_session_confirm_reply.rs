//! c14 / scope §CT5 happy path: a `frontend.tui.confirm_answer` with
//! `answer = "allow"` and a `Held` entry in `ConfirmState` is
//! canonicalised to `core.session.confirm_reply` with the payload
//! `request_id` forwarded as the correlation id, `in_reply_to` set to
//! `[correlation_id]`, and canonical `user` taint per §CT5 step 7.

use std::sync::Arc;
use std::time::Duration;

use rafaello_core::bus::JsonRpcId;
use rafaello_core::gate::ConfirmState;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::confirm_state_kit::held;
use common::reemit_test_kit::{
    assert_origin_taint, await_topic, build_rig, subscribe_core_test_receiver, AuditRig, RigOpts,
    TUI_ATTACH_ID,
};

#[tokio::test]
async fn frontend_confirm_answer_reemitted_as_core_session_confirm_reply() {
    let rig = build_rig(RigOpts {
        include_tui_frontend: true,
        tui_publish_confirm_answer: true,
        ..Default::default()
    });
    let attach = rig.frontend_attach.clone().expect("tui registered");
    let audit_rig = AuditRig::new(&rig.broker);

    let state = Arc::new(ConfirmState::new());
    let correlation = JsonRpcId::from(ulid::Ulid::new().to_string());
    state.reserve(correlation.clone(), held());

    let (mut canonical_rx, _csub) = subscribe_core_test_receiver(&rig.broker);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let router = ReemitRouter::new(
        rig.broker.clone(),
        rig.acl.clone(),
        rig.provider_canonical.clone(),
        shutdown_rx,
    )
    .with_confirm_state_and_audit(state.clone(), audit_rig.writer.clone());
    let join = router.start();

    let envelope_id = JsonRpcId::from(ulid::Ulid::new().to_string());
    let params = serde_json::json!({
        "topic": format!("frontend.{TUI_ATTACH_ID}.confirm_answer"),
        "payload": {"request_id": correlation.to_string(), "answer": "allow"},
        "in_reply_to": [correlation.clone()],
        "request_id": envelope_id,
    });
    rig.broker
        .handle_frontend_publish(&attach, &params)
        .expect("publish accepted");

    let mut seen = Vec::new();
    let event = await_topic(&mut canonical_rx, "core.session.confirm_reply", &mut seen).await;
    assert_origin_taint(&event, "user", None);
    assert_eq!(event.payload["request_id"], correlation.to_string());
    assert_eq!(event.payload["answer"], "allow");
    assert_eq!(
        event.in_reply_to,
        Some(vec![correlation.clone()]),
        "in_reply_to carries the correlation id"
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
