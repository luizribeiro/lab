//! Scope §TR1 ordering pin (c10, pi-4 N-1): the handler's
//! `taint_match.record(...)` runs BEFORE `publish_core_with_taint`, so
//! any subscriber — internal or external — that observes the canonical
//! event finds the map already populated. We assert this with c08's
//! `Broker::install_publish_test_hook`, which fires inside
//! `publish_core_with_taint` after event construction but before
//! `fan_out`: at hook-fire time the recorded entry must already be
//! visible.

#![cfg(feature = "test-fixture")]

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::reemit::taint_match::TaintMatchMap;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{build_rig, RigOpts, READFILE_CANONICAL, READFILE_TOPIC_ID};

#[tokio::test]
async fn reemit_tool_result_records_before_publish() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        ..Default::default()
    });
    let plugin_canonical = rig.readfile_canonical.clone().expect("readfile present");

    let shared = Arc::new(TaintMatchMap::new(Duration::from_secs(300), 16));
    let verbatim = "verbatim string above sixteen bytes";

    let captured: Arc<Mutex<Option<Vec<TaintEntry>>>> = Arc::new(Mutex::new(None));
    let captured_for_hook = captured.clone();
    let shared_for_hook = shared.clone();
    let lookup_args = serde_json::json!({"arg": verbatim});
    rig.broker.install_publish_test_hook(Arc::new(move |event| {
        if event.topic == "core.session.tool_result" {
            let hits = shared_for_hook.lookup(&lookup_args);
            *captured_for_hook.lock() = Some(hits);
        }
        None
    }));

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

    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let hits = loop {
        if let Some(h) = captured.lock().clone() {
            break h;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("publish test hook never fired for core.session.tool_result");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    };

    let expected = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some(READFILE_CANONICAL.to_string()),
    }];
    assert_eq!(
        hits, expected,
        "TaintMatchMap must already contain the recorded entry at hook-fire time \
         (record runs before publish_core_with_taint)",
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
