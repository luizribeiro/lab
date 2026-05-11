//! Explicit user-taint synthesis on CR5 (pi-1 L-3): even if the
//! TUI's inbound publish carries no taint, the canonical event has
//! `taint = [{source: "user", detail: None}]`. m4 is the only point
//! in v1 where the `"user"` taxon originates.

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
async fn user_message_synthesises_user_taint() {
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
        "payload": {"text": "from the user"},
        "request_id": JsonRpcId::from("ulid-synth"),
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
