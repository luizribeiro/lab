//! c14 / scope §CT5 step 4 — `Unknown` branch: a well-formed
//! `frontend.tui.confirm_answer` for a `request_id` that was never
//! held audits `confirm_unknown` and drops; no
//! `core.session.confirm_reply` is emitted.

use std::sync::Arc;
use std::time::Duration;

use rafaello_core::bus::JsonRpcId;
use rafaello_core::gate::ConfirmState;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{
    build_rig, drain_for, subscribe_core_test_receiver, AuditRig, RigOpts, TUI_ATTACH_ID,
};

#[tokio::test]
async fn confirm_answer_unknown_request_id_audit_logged() {
    let rig = build_rig(RigOpts {
        include_tui_frontend: true,
        tui_publish_confirm_answer: true,
        ..Default::default()
    });
    let attach = rig.frontend_attach.clone().expect("tui registered");
    let audit_rig = AuditRig::new(&rig.broker);

    let state = Arc::new(ConfirmState::new());
    let unknown_id = JsonRpcId::from(ulid::Ulid::new().to_string());

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
        "payload": {"request_id": unknown_id.to_string(), "answer": "allow"},
        "in_reply_to": [unknown_id.clone()],
        "request_id": JsonRpcId::from(ulid::Ulid::new().to_string()),
    });
    rig.broker
        .handle_frontend_publish(&attach, &params)
        .expect("publish accepted");

    let observed = drain_for(&mut canonical_rx, Duration::from_millis(150)).await;
    for ev in &observed {
        assert_ne!(
            ev.topic, "core.session.confirm_reply",
            "no confirm_reply on Unknown branch"
        );
    }

    let rows = audit_rig.rows();
    assert_eq!(rows.len(), 1, "exactly one audit row written");
    assert_eq!(rows[0].1, "confirm_unknown");
    assert_eq!(rows[0].2.as_deref(), Some(unknown_id.to_string().as_str()));

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
