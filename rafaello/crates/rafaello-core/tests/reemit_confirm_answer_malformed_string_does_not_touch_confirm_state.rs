//! c14 / scope §CT5 step 3 (pi-3 M-3): a `frontend.tui.confirm_answer`
//! with `answer` not in `{allow, deny, always_allow_session}` fails
//! with `ReemitError::ConfirmAnswerMalformed`, audits
//! `confirm_malformed`, and **never touches `ConfirmState`** — the
//! held entry remains `Active` so CG5's deadline timer can resolve it
//! on schedule.

use std::sync::Arc;
use std::time::Duration;

use rafaello_core::bus::JsonRpcId;
use rafaello_core::gate::{ConfirmState, PriorOutcome};
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::confirm_state_kit::held;
use common::reemit_test_kit::{
    build_rig, drain_for, subscribe_core_test_receiver, AuditRig, RigOpts, TUI_ATTACH_ID,
};

#[tokio::test]
async fn confirm_answer_malformed_string_does_not_touch_confirm_state() {
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

    assert_eq!(
        state.prior_outcome(&correlation),
        PriorOutcome::Held,
        "precondition: entry is Active before malformed answer"
    );

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
        "payload": {"request_id": correlation.to_string(), "answer": "maybe-later"},
        "in_reply_to": [correlation.clone()],
        "request_id": JsonRpcId::from(ulid::Ulid::new().to_string()),
    });
    rig.broker
        .handle_frontend_publish(&attach, &params)
        .expect("publish accepted");

    // Drain a bit; the §CR7 failure path emits
    // `core.lifecycle.reemit_rejected` but no `confirm_reply`.
    let observed = drain_for(&mut canonical_rx, Duration::from_millis(150)).await;
    for ev in &observed {
        assert_ne!(ev.topic, "core.session.confirm_reply");
    }

    assert_eq!(
        state.prior_outcome(&correlation),
        PriorOutcome::Held,
        "ConfirmState must be untouched after malformed answer (pi-3 M-3)"
    );

    let rows = audit_rig.rows();
    let kinds: Vec<&str> = rows.iter().map(|r| r.1.as_str()).collect();
    assert!(
        kinds.contains(&"confirm_malformed"),
        "audit log records `confirm_malformed`; got {kinds:?}"
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
