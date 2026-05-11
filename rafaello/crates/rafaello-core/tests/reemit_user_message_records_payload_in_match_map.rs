//! Scope §TR2 (c10): when the re-emit router fires
//! `core.session.user_message`, the canonical user-source taint is
//! recorded into the router-owned `TaintMatchMap` so a later lookup
//! whose args reference any scalar leaf of the original user message
//! finds the taint.

#![cfg(feature = "test-fixture")]

use std::sync::Arc;
use std::time::Duration;

use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::reemit::taint_match::TaintMatchMap;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{
    await_topic, build_rig, subscribe_core_test_receiver, RigOpts, TUI_ATTACH_ID,
};

#[tokio::test]
async fn reemit_user_message_records_payload_in_match_map() {
    let rig = build_rig(RigOpts {
        include_tui_frontend: true,
        ..Default::default()
    });
    let attach = rig.frontend_attach.clone().expect("tui registered");

    let (mut canonical_rx, _csub) = subscribe_core_test_receiver(&rig.broker);

    let shared = Arc::new(TaintMatchMap::new(Duration::from_secs(300), 16));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let router = ReemitRouter::new(
        rig.broker.clone(),
        rig.acl.clone(),
        rig.provider_canonical.clone(),
        shutdown_rx,
    )
    .with_taint_match_map(shared.clone());
    let join = router.start();

    let verbatim = "verbatim string above sixteen bytes";
    let params = serde_json::json!({
        "topic": format!("frontend.{TUI_ATTACH_ID}.user_message"),
        "payload": {"text": verbatim},
        "request_id": JsonRpcId::from("ulid-synth"),
    });
    rig.broker
        .handle_frontend_publish(&attach, &params)
        .expect("publish accepted");

    let mut seen = Vec::new();
    let _event = await_topic(&mut canonical_rx, "core.session.user_message", &mut seen).await;

    let hits = shared.lookup(&serde_json::json!({"arg": verbatim}));
    let expected = vec![TaintEntry {
        source: "user".to_string(),
        detail: None,
    }];
    assert_eq!(
        hits, expected,
        "user-source canonical taint recorded for user_message payload",
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
