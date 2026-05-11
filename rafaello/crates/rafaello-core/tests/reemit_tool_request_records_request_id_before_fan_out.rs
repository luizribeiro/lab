//! Scope §TR3 step 6 ordering pin (c11, pi-4 B-1): the handler's
//! `referenced_taint_index.record_request(...)` runs BEFORE
//! `publish_core_with_taint`, so any subscriber — internal or external —
//! that observes the canonical event finds the `by_request_id` arm
//! already populated. We assert this with c08's
//! `Broker::install_publish_test_hook`, which fires inside
//! `publish_core_with_taint` after event construction but before
//! `fan_out`: at hook-fire time the recorded entry must already be
//! visible.

#![cfg(feature = "test-fixture")]

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::reemit::referenced_taint_index::ReferencedTaintIndex;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{build_rig, RigOpts, MOCK_PROVIDER_ID, READFILE_CANONICAL};

#[tokio::test]
async fn reemit_tool_request_records_request_id_before_fan_out() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        tool_routes: vec![("readfile", READFILE_CANONICAL)],
        ..Default::default()
    });

    let prior_result = JsonRpcId::from("prior-1");
    rig.broker
        .seed_provider_observed_result_for_test(&rig.provider_canonical, prior_result.clone());

    let shared = Arc::new(ReferencedTaintIndex::new(Duration::from_secs(300)));
    let request_id = JsonRpcId::from("req-1");

    let captured: Arc<Mutex<Option<Vec<TaintEntry>>>> = Arc::new(Mutex::new(None));
    let captured_for_hook = captured.clone();
    let shared_for_hook = shared.clone();
    let lookup_id = request_id.clone();
    rig.broker.install_publish_test_hook(Arc::new(move |event| {
        if event.topic == "core.session.tool_request" {
            let hit = shared_for_hook.lookup_request(&lookup_id);
            *captured_for_hook.lock() = Some(hit.unwrap_or_default());
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
    .with_referenced_taint_index(shared.clone());
    let join = router.start();

    let params = serde_json::json!({
        "topic": format!("provider.{MOCK_PROVIDER_ID}.tool_request"),
        "payload": {"tool": "readfile", "args": {"path": "src/main.rs"}},
        "in_reply_to": [prior_result],
        "request_id": request_id.clone(),
    });
    rig.broker
        .handle_provider_publish(&rig.provider_canonical, &params)
        .expect("publish accepted");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let hit = loop {
        if let Some(h) = captured.lock().clone() {
            break h;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("publish test hook never fired for core.session.tool_request");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    };

    let expected = vec![TaintEntry {
        source: "provider".to_string(),
        detail: Some(MOCK_PROVIDER_ID.to_string()),
    }];
    assert_eq!(
        hit, expected,
        "ReferencedTaintIndex.by_request_id must already contain the recorded \
         entry at hook-fire time (record_request runs before publish_core_with_taint)",
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
