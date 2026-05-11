//! pi-1 L-3 negative half: the TUI publishes `frontend.tui.user_message`
//! with an inbound `taint: [{source: "provider"}]`. The canonical
//! `core.session.user_message` ignores the inbound taint entirely and
//! carries `[{source: "user", detail: None}]` only.

use std::time::Duration;

use rafaello_core::bus::JsonRpcId;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{
    assert_origin_taint, await_topic, build_rig, subscribe_core_test_receiver, RigOpts,
    TUI_ATTACH_ID,
};

#[tokio::test]
async fn user_message_discards_frontend_supplied_taint() {
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
        "payload": {"text": "spoofed"},
        "request_id": JsonRpcId::from("ulid-spoof"),
        "taint": [{"source": "provider", "detail": "evil"}],
    });
    rig.broker
        .handle_frontend_publish(&attach, &params)
        .expect("publish accepted");

    let mut seen = Vec::new();
    let event = await_topic(&mut canonical_rx, "core.session.user_message", &mut seen).await;
    assert_origin_taint(&event, "user", None);

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
