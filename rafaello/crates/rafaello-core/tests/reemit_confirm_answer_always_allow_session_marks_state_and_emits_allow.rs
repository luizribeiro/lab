//! c14 / scope §CT5 step 5 — `always_allow_session` happy path
//! (pi-3 B-2 + pi-5 N-2): re-emit calls
//! `ConfirmState::mark_session_grant_requested`, flips the
//! `session_grant_requested` flag, and rewrites the outbound
//! `confirm_reply.payload.answer` to the two-value enum `"allow"`.
//! The held entry stays `Active` (CG4 still consumes it later).

use std::sync::Arc;
use std::time::Duration;

use rafaello_core::bus::JsonRpcId;
use rafaello_core::gate::ConfirmState;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::confirm_state_kit::held;
use common::reemit_test_kit::{
    await_topic, build_rig, subscribe_core_test_receiver, AuditRig, RigOpts, TUI_ATTACH_ID,
};

#[tokio::test]
async fn confirm_answer_always_allow_session_marks_state_and_emits_allow() {
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

    let params = serde_json::json!({
        "topic": format!("frontend.{TUI_ATTACH_ID}.confirm_answer"),
        "payload": {"request_id": correlation.to_string(), "answer": "always_allow_session"},
        "in_reply_to": [correlation.clone()],
        "request_id": JsonRpcId::from(ulid::Ulid::new().to_string()),
    });
    rig.broker
        .handle_frontend_publish(&attach, &params)
        .expect("publish accepted");

    let mut seen = Vec::new();
    let event = await_topic(&mut canonical_rx, "core.session.confirm_reply", &mut seen).await;
    assert_eq!(
        event.payload["answer"], "allow",
        "outbound answer rewritten to the two-value enum"
    );
    assert_eq!(event.payload["request_id"], correlation.to_string());

    // CG4's try_resolve must still see Active with the
    // session_grant_requested flag flipped.
    let resolved = state.try_resolve(&correlation).expect("entry stays Active");
    assert!(
        resolved.1,
        "session_grant_requested flipped by mark_session_grant_requested"
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
