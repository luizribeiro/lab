//! Scope §TR1 ancestry-union / §PT2 (c13): the canonical
//! `core.session.tool_result` envelope carries taint = tool-source ∪
//! referenced-request-taint, deduped and deterministically sorted.

#![cfg(feature = "test-fixture")]

use std::sync::Arc;
use std::time::Duration;

use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::reemit::referenced_taint_index::ReferencedTaintIndex;
use rafaello_core::reemit::taint_match::TaintMatchMap;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{
    await_topic, build_rig, subscribe_core_test_receiver, RigOpts, READFILE_CANONICAL,
    READFILE_TOPIC_ID,
};

#[tokio::test]
async fn reemit_tool_result_canonical_taint_unions_request_ancestry() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        ..Default::default()
    });
    let plugin_canonical = rig.readfile_canonical.clone().expect("readfile present");

    let referenced = Arc::new(ReferencedTaintIndex::new(Duration::from_secs(300)));
    let taint_match = Arc::new(TaintMatchMap::new(Duration::from_secs(300), 16));

    let tool_request_id = JsonRpcId::from("tool-req-1");
    let request_taint = vec![
        TaintEntry {
            source: "provider".to_string(),
            detail: Some("openai".to_string()),
        },
        TaintEntry {
            source: "tool".to_string(),
            detail: Some("local/test:fetch@0.1.0".to_string()),
        },
    ];
    referenced.record_request(&tool_request_id, &request_taint);

    let (mut canonical_rx, _csub) = subscribe_core_test_receiver(&rig.broker);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let router = ReemitRouter::new(
        rig.broker.clone(),
        rig.acl.clone(),
        rig.provider_canonical.clone(),
        shutdown_rx,
    )
    .with_taint_match_map(taint_match.clone())
    .with_referenced_taint_index(referenced.clone());
    let join = router.start();

    let result_id = JsonRpcId::from("res-1");
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
        "payload": {"content": "anything"},
        "in_reply_to": [tool_request_id],
        "request_id": result_id,
    });
    rig.broker
        .handle_plugin_publish(&plugin_canonical, &params)
        .expect("publish accepted");

    let mut seen = Vec::new();
    let canonical = await_topic(&mut canonical_rx, "core.session.tool_result", &mut seen).await;
    let taint = canonical.taint.as_ref().expect("taint present");

    let mut expected = vec![
        TaintEntry {
            source: "provider".to_string(),
            detail: Some("openai".to_string()),
        },
        TaintEntry {
            source: "tool".to_string(),
            detail: Some("local/test:fetch@0.1.0".to_string()),
        },
        TaintEntry {
            source: "tool".to_string(),
            detail: Some(READFILE_CANONICAL.to_string()),
        },
    ];
    expected.sort_by(|a, b| {
        (a.source.as_str(), a.detail.as_deref()).cmp(&(b.source.as_str(), b.detail.as_deref()))
    });
    assert_eq!(taint, &expected);

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
