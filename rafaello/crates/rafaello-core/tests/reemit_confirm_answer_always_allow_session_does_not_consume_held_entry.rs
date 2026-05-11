//! c14 / scope §CT5 step 5 (pi-3 B-1 ownership): on the
//! `always_allow_session` happy path, re-emit *marks* the
//! `session_grant_requested` flag but **does not consume** the held
//! entry. A subsequent `try_resolve` (CG4's responsibility) still
//! returns `Some((held, true))`.

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
async fn confirm_answer_always_allow_session_does_not_consume_held_entry() {
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
    let _reply = await_topic(&mut canonical_rx, "core.session.confirm_reply", &mut seen).await;

    // CG4's try_resolve: still returns Active + flagged.
    let resolved = state
        .try_resolve(&correlation)
        .expect("entry is still Active after re-emit step 5");
    assert!(
        resolved.1,
        "session_grant_requested == true (flag was flipped by re-emit)"
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
