//! Scope §TR1 ordering pin (c13, pi-4 N-1): both
//! `TaintMatchMap::record` (step 5) and
//! `ReferencedTaintIndex::record_result` (step 6) complete strictly
//! BEFORE `publish_core_with_taint`, so any subscriber that observes the
//! canonical `core.session.tool_result` finds both indexes populated.
//! We assert this with c08's `Broker::install_publish_test_hook`, which
//! fires inside `publish_core_with_taint` after event construction but
//! before `fan_out`.

#![cfg(feature = "test-fixture")]

use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::reemit::referenced_taint_index::ReferencedTaintIndex;
use rafaello_core::reemit::taint_match::TaintMatchMap;
use rafaello_core::reemit::ReemitRouter;
use tokio::sync::watch;

mod common;
use common::reemit_test_kit::{build_rig, RigOpts, READFILE_CANONICAL, READFILE_TOPIC_ID};

#[tokio::test]
async fn reemit_tool_result_records_both_indexes_before_fan_out() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        ..Default::default()
    });
    let plugin_canonical = rig.readfile_canonical.clone().expect("readfile present");

    let taint_match = Arc::new(TaintMatchMap::new(Duration::from_secs(300), 16));
    let referenced = Arc::new(ReferencedTaintIndex::new(Duration::from_secs(300)));

    let tool_request_id = JsonRpcId::from("tool-req-1");
    let result_id = JsonRpcId::from("res-1");
    let prior_user_taint = vec![TaintEntry {
        source: "user".to_string(),
        detail: None,
    }];
    referenced.record_request(&tool_request_id, &prior_user_taint);

    let verbatim = "verbatim string above sixteen bytes";

    let captured_match: Arc<Mutex<Option<Vec<TaintEntry>>>> = Arc::new(Mutex::new(None));
    let captured_result: Arc<Mutex<Option<Vec<TaintEntry>>>> = Arc::new(Mutex::new(None));
    let captured_match_for_hook = captured_match.clone();
    let captured_result_for_hook = captured_result.clone();
    let taint_match_for_hook = taint_match.clone();
    let referenced_for_hook = referenced.clone();
    let lookup_args = serde_json::json!({"arg": verbatim});
    let lookup_id = result_id.clone();
    rig.broker.install_publish_test_hook(Arc::new(move |event| {
        if event.topic == "core.session.tool_result" {
            *captured_match_for_hook.lock() = Some(taint_match_for_hook.lookup(&lookup_args));
            *captured_result_for_hook.lock() = Some(
                referenced_for_hook
                    .lookup_result(&lookup_id)
                    .unwrap_or_default(),
            );
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
    .with_taint_match_map(taint_match.clone())
    .with_referenced_taint_index(referenced.clone());
    let join = router.start();

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
        "payload": {"content": verbatim},
        "in_reply_to": [tool_request_id],
        "request_id": result_id,
    });
    rig.broker
        .handle_plugin_publish(&plugin_canonical, &params)
        .expect("publish accepted");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    let (match_hits, result_hits) = loop {
        let m = captured_match.lock().clone();
        let r = captured_result.lock().clone();
        if let (Some(m), Some(r)) = (m, r) {
            break (m, r);
        }
        if tokio::time::Instant::now() >= deadline {
            panic!("publish test hook never fired for core.session.tool_result");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    };

    let mut expected = vec![
        TaintEntry {
            source: "tool".to_string(),
            detail: Some(READFILE_CANONICAL.to_string()),
        },
        TaintEntry {
            source: "user".to_string(),
            detail: None,
        },
    ];
    expected.sort_by(|a, b| {
        (a.source.as_str(), a.detail.as_deref()).cmp(&(b.source.as_str(), b.detail.as_deref()))
    });
    assert_eq!(
        match_hits, expected,
        "TaintMatchMap must already contain the unioned entry at hook-fire time",
    );
    assert_eq!(
        result_hits, expected,
        "ReferencedTaintIndex.by_result_id must already contain the unioned entry at hook-fire time",
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
