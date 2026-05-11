//! c14 / scope §CT5 + CT0 implication 2: on
//! `frontend.tui.confirm_answer` the envelope `request_id` and the
//! payload `request_id` are distinct (Stream A semantics — payload is
//! the *correlation id*, envelope is a fresh ULID). The canonical
//! `core.session.confirm_reply` preserves that distinction: the
//! envelope `request_id` is the inbound envelope id (forwarded by
//! re-emit), the payload `request_id` is the correlation id.

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
async fn confirm_answer_payload_id_neq_envelope_id() {
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
    assert_ne!(envelope_id, correlation, "test setup: ids must differ");

    let params = serde_json::json!({
        "topic": format!("frontend.{TUI_ATTACH_ID}.confirm_answer"),
        "payload": {"request_id": correlation.to_string(), "answer": "deny"},
        "in_reply_to": [correlation.clone()],
        "request_id": envelope_id.clone(),
    });
    rig.broker
        .handle_frontend_publish(&attach, &params)
        .expect("publish accepted");

    let mut seen = Vec::new();
    let event = await_topic(&mut canonical_rx, "core.session.confirm_reply", &mut seen).await;
    assert_eq!(
        event.request_id,
        Some(envelope_id),
        "envelope request_id forwarded verbatim"
    );
    assert_eq!(
        event.payload["request_id"],
        correlation.to_string(),
        "payload request_id is the correlation id, distinct from envelope"
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
