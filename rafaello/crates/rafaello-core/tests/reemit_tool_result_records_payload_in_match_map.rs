//! Scope §TR1 (refresh-map half, c10): when the re-emit router fires
//! `core.session.tool_result`, the canonical tool-source taint is
//! recorded into the router-owned `TaintMatchMap` so a later lookup
//! whose args reference any scalar leaf of the original tool-result
//! payload finds the taint. c13 will extend this recorded vector with
//! the ancestry union; c10 asserts the tool-source-only shape.

#![cfg(feature = "test-fixture")]

use std::sync::Arc;
use std::time::Duration;

use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::reemit::taint_match::TaintMatchMap;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{
    await_topic, build_rig, subscribe_core_test_receiver, RigOpts, READFILE_CANONICAL,
    READFILE_TOPIC_ID,
};

#[tokio::test]
async fn reemit_tool_result_records_payload_in_match_map() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        ..Default::default()
    });
    let plugin_canonical = rig.readfile_canonical.clone().expect("readfile present");

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

    let tool_request_id = JsonRpcId::from("tool-req-1");
    let request_id = JsonRpcId::from("res-1");
    let verbatim = "verbatim string above sixteen bytes";
    let inbound_payload = serde_json::json!({"content": verbatim});
    rig.broker
        .publish_for_tool_dispatch(
            &plugin_canonical,
            serde_json::json!({}),
            tool_request_id.clone(),
            None,
            None,
            Vec::new(),
        )
        .expect("dispatch seeds outstanding map");
    let params = serde_json::json!({
        "topic": format!("plugin.{READFILE_TOPIC_ID}.tool_result"),
        "payload": inbound_payload,
        "in_reply_to": [tool_request_id],
        "request_id": request_id,
    });
    rig.broker
        .handle_plugin_publish(&plugin_canonical, &params)
        .expect("publish accepted");

    let mut seen = Vec::new();
    let _event = await_topic(&mut canonical_rx, "core.session.tool_result", &mut seen).await;

    let hits = shared.lookup(&serde_json::json!({"arg": verbatim}));
    let expected = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some(READFILE_CANONICAL.to_string()),
    }];
    assert_eq!(
        hits, expected,
        "tool-source-only canonical taint recorded for tool_result payload (c13 will extend)",
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
