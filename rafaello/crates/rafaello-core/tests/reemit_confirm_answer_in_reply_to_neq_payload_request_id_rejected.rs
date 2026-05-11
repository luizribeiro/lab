//! c14 / scope §CT5 step 2: `in_reply_to` must equal
//! `[payload.request_id]`. When they disagree, re-emit fails with
//! `ReemitError::ConfirmAnswerCorrelationMismatch` (surfaced via the
//! §CR7 `core.lifecycle.reemit_rejected` observability event), never
//! touches `ConfirmState`, and emits no `core.session.confirm_reply`.

use std::sync::Arc;
use std::time::Duration;

use rafaello_core::bus::JsonRpcId;
use rafaello_core::gate::{ConfirmState, PriorOutcome};
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::confirm_state_kit::held;
use common::reemit_test_kit::{
    await_topic, build_rig, drain_for, subscribe_core_test_receiver, AuditRig, RigOpts,
    TUI_ATTACH_ID,
};

#[tokio::test]
async fn confirm_answer_in_reply_to_neq_payload_request_id_rejected() {
    let rig = build_rig(RigOpts {
        include_tui_frontend: true,
        tui_publish_confirm_answer: true,
        ..Default::default()
    });
    let attach = rig.frontend_attach.clone().expect("tui registered");
    let audit_rig = AuditRig::new(&rig.broker);

    let state = Arc::new(ConfirmState::new());
    let correlation = JsonRpcId::from(ulid::Ulid::new().to_string());
    let other = JsonRpcId::from(ulid::Ulid::new().to_string());
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
        "payload": {"request_id": correlation.to_string(), "answer": "allow"},
        "in_reply_to": [other],
        "request_id": JsonRpcId::from(ulid::Ulid::new().to_string()),
    });
    rig.broker
        .handle_frontend_publish(&attach, &params)
        .expect("publish accepted by broker (envelope shape is valid)");

    let mut seen = Vec::new();
    let rejected = await_topic(
        &mut canonical_rx,
        "core.lifecycle.reemit_rejected",
        &mut seen,
    )
    .await;
    let reason = rejected.payload["reason"]
        .as_str()
        .expect("reason is a string");
    assert!(
        reason.contains("correlation mismatch") || reason.contains("Correlation"),
        "reason names the §CT5 correlation-mismatch failure: {reason}"
    );

    for ev in &seen {
        assert_ne!(ev.topic, "core.session.confirm_reply");
    }
    let tail = drain_for(&mut canonical_rx, Duration::from_millis(100)).await;
    for ev in &tail {
        assert_ne!(ev.topic, "core.session.confirm_reply");
    }

    assert_eq!(
        state.prior_outcome(&correlation),
        PriorOutcome::Held,
        "ConfirmState must not be touched on correlation mismatch (step 2)"
    );
    assert!(
        audit_rig.rows().is_empty(),
        "no audit row written on correlation mismatch"
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
