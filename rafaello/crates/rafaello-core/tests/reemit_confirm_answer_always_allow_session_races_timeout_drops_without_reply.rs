//! c14 / scope §CT5 step 5 — pi-5 M-1 race coverage: between the
//! step-4 `prior_outcome == Held` read and the step-5
//! `mark_session_grant_requested` call, CG5's deadline timer fires
//! and consumes the entry. `mark_session_grant_requested` returns
//! `MarkError::NotActive`; re-emit re-reads `prior_outcome`, audits
//! `confirm_late`, and emits **no** `core.session.confirm_reply`. The
//! test seam (`with_test_confirm_race_hook`) deterministically
//! interleaves the timeout between the two calls.

#![cfg(feature = "test-fixture")]

use std::sync::Arc;
use std::time::Duration;

use rafaello_core::bus::JsonRpcId;
use rafaello_core::gate::ConfirmState;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::confirm_state_kit::held;
use common::reemit_test_kit::{
    build_rig, drain_for, subscribe_core_test_receiver, AuditRig, RigOpts, TUI_ATTACH_ID,
};

#[tokio::test]
async fn confirm_answer_always_allow_session_races_timeout_drops_without_reply() {
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

    let race_state = state.clone();
    let race_id = correlation.clone();
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let router = ReemitRouter::new(
        rig.broker.clone(),
        rig.acl.clone(),
        rig.provider_canonical.clone(),
        shutdown_rx,
    )
    .with_confirm_state_and_audit(state.clone(), audit_rig.writer.clone())
    .with_test_confirm_race_hook(Arc::new(move || {
        // Simulate CG5's deadline winning the race: move the entry
        // to `TimedOut` after re-emit's step-4 `prior_outcome == Held`
        // read but before its step-5 `mark_session_grant_requested`.
        let _ = race_state.try_take_for_timeout(&race_id);
    }));
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

    let observed = drain_for(&mut canonical_rx, Duration::from_millis(150)).await;
    for ev in &observed {
        assert_ne!(
            ev.topic, "core.session.confirm_reply",
            "no canonical reply when the timeout wins the race"
        );
    }

    let rows = audit_rig.rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].1, "confirm_late");

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
