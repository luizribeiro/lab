//! Round-4 (pi-3 B-1): the broker rejects a frontend
//! `frontend.tui.user_message` publish that arrives without
//! `request_id` (§B0). Confirm the canonical re-emit does NOT fire
//! in that case — no `core.session.user_message` reaches a subscriber
//! on `core.session.**`.

use std::time::Duration;

use rafaello_core::error::Publisher;
use rafaello_core::reemit::ReemitRouter;
use rafaello_core::BrokerError;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{
    build_rig, drain_for, subscribe_core_test_receiver, RigOpts, TUI_ATTACH_ID,
};

#[tokio::test]
async fn frontend_user_message_missing_request_id_does_not_reemit() {
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

    let params = serde_json::json!({
        "topic": format!("frontend.{TUI_ATTACH_ID}.user_message"),
        "payload": {"text": "no rid"},
    });
    let err = rig
        .broker
        .handle_frontend_publish(&attach, &params)
        .expect_err("broker rejects missing request_id");
    assert!(
        matches!(
            err,
            BrokerError::MissingRequestId {
                publisher: Publisher::Frontend(_),
                ..
            }
        ),
        "expected MissingRequestId{{Frontend}}, got {err:?}"
    );

    let observed = drain_for(&mut canonical_rx, Duration::from_millis(200)).await;
    for ev in &observed {
        assert_ne!(
            ev.topic, "core.session.user_message",
            "canonical re-emit must not fire when broker rejects inbound"
        );
    }

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
