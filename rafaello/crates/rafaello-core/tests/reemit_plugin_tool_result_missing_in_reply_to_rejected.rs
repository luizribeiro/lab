//! The broker already rejects a `plugin.<topic-id>.tool_result` publish
//! with no `in_reply_to` as `InvalidInReplyTo { Missing }` — this
//! confirms the canonical re-emit does NOT fire for such an event
//! (no `core.session.tool_result` reaches a subscriber on
//! `core.session.**`).

use std::time::Duration;

use rafaello_core::bus::JsonRpcId;
use rafaello_core::error::{InReplyToReason, Publisher};
use rafaello_core::reemit::ReemitRouter;
use rafaello_core::BrokerError;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{
    build_rig, drain_for, subscribe_core_test_receiver, RigOpts, READFILE_TOPIC_ID,
};

#[tokio::test]
async fn plugin_tool_result_missing_in_reply_to_does_not_reemit() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        ..Default::default()
    });
    let plugin_canonical = rig.readfile_canonical.clone().expect("readfile present");

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
        "topic": format!("plugin.{READFILE_TOPIC_ID}.tool_result"),
        "payload": {"ok": true, "content": ""},
        "request_id": JsonRpcId::from("res-no-irt"),
    });
    let err = rig
        .broker
        .handle_plugin_publish(&plugin_canonical, &params)
        .expect_err("broker rejects missing in_reply_to");
    assert!(
        matches!(
            err,
            BrokerError::InvalidInReplyTo {
                publisher: Publisher::Plugin(_),
                reason: InReplyToReason::Missing,
                ..
            }
        ),
        "expected InvalidInReplyTo{{Plugin, Missing}}, got {err:?}"
    );

    let observed = drain_for(&mut canonical_rx, Duration::from_millis(200)).await;
    for ev in &observed {
        assert_ne!(
            ev.topic, "core.session.tool_result",
            "canonical re-emit must not fire when broker rejects inbound"
        );
    }

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
