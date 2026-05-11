//! Moved from c15 (pi-1 B-4). Drive a frontend `bus.publish` for
//! `frontend.tui.user_message` with a valid `request_id`. The
//! router's `subscribe_internal` receiver observes the inbound event,
//! AND the broker subsequently fans out the canonical
//! `core.session.user_message` to an **external** subscriber
//! registered via `register_plugin` with
//! `subscribe_patterns = ["core.session.**"]`.

use std::time::Duration;

use rafaello_core::broker_acl::PluginAcl;
use rafaello_core::bus::JsonRpcId;
use rafaello_core::lock::CanonicalId;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::peer_test_kit::fresh_peer;
use common::reemit_test_kit::{
    assert_origin_taint, await_topic, build_rig, subscribe_router_test_receiver, RigOpts,
    TUI_ATTACH_ID,
};

#[tokio::test]
async fn frontend_publish_user_message_reemitted_as_core_session_user_message() {
    let observer = CanonicalId::parse("local/test:obs@0.1.0").expect("canonical");
    let observer_acl = PluginAcl {
        topic_id: "obs_local_test".to_string(),
        publish_topics: vec![],
        subscribe_patterns: vec!["core.session.**".to_string()],
        auto_subscribes: vec![],
        provider_id: None,
    };

    let rig = build_rig(RigOpts {
        include_tui_frontend: true,
        extra_plugins: vec![(observer.clone(), observer_acl)],
        ..Default::default()
    });
    let attach = rig.frontend_attach.clone().expect("tui registered");

    let (peer_obs, mut rx_obs) = fresh_peer();
    let _g_obs = rig
        .broker
        .register_plugin(observer.clone(), peer_obs)
        .expect("observer registers");

    let (mut router_rx, _rsub) = subscribe_router_test_receiver(&rig.broker);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let router = ReemitRouter::new(
        rig.broker.clone(),
        rig.acl.clone(),
        rig.provider_canonical.clone(),
        shutdown_rx,
    );
    let join = router.start();

    let request_id = JsonRpcId::from("ulid-c15");
    let inbound_topic = format!("frontend.{TUI_ATTACH_ID}.user_message");
    let params = serde_json::json!({
        "topic": inbound_topic,
        "payload": {"text": "round-trip"},
        "request_id": request_id.clone(),
    });
    rig.broker
        .handle_frontend_publish(&attach, &params)
        .expect("publish accepted");

    let mut seen = Vec::new();
    let inbound_event = await_topic(&mut router_rx, "frontend.tui.user_message", &mut seen).await;
    assert_eq!(inbound_event.payload["text"], "round-trip");

    let notification = tokio::time::timeout(Duration::from_secs(2), rx_obs.recv())
        .await
        .expect("observer received notification")
        .expect("notify channel still open");
    assert_eq!(notification.method, "bus.event");
    let canonical = &notification.params;
    assert_eq!(canonical["topic"], "core.session.user_message");
    assert_eq!(canonical["payload"]["text"], "round-trip");
    assert_eq!(canonical["request_id"], serde_json::json!("ulid-c15"));
    let taint = canonical["taint"]
        .as_array()
        .expect("canonical event carries taint array");
    assert_eq!(taint.len(), 1);
    assert_eq!(taint[0]["source"], "user");
    assert!(taint[0]["detail"].is_null());

    // Belt-and-braces: also confirm the canonical event reached an
    // internal subscriber with the expected taint shape via the
    // typed `BusEvent`, not just the JSON wire form.
    let mut seen2 = Vec::new();
    let (mut canonical_rx, _csub) =
        common::reemit_test_kit::subscribe_core_test_receiver(&rig.broker);
    // Re-publish to exercise both the external (already asserted)
    // and the internal-subscriber path on a second publish so we
    // get a fresh BusEvent into `canonical_rx`.
    let request_id2 = JsonRpcId::from("ulid-c15-2");
    let params2 = serde_json::json!({
        "topic": format!("frontend.{TUI_ATTACH_ID}.user_message"),
        "payload": {"text": "round-trip-2"},
        "request_id": request_id2.clone(),
    });
    rig.broker
        .handle_frontend_publish(&attach, &params2)
        .expect("second publish accepted");
    let canonical_event =
        await_topic(&mut canonical_rx, "core.session.user_message", &mut seen2).await;
    assert_origin_taint(&canonical_event, "user", None);
    assert_eq!(canonical_event.request_id, Some(request_id2));

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
