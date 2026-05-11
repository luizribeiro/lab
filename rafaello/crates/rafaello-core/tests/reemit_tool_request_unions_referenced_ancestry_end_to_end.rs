//! Scope §TR1 + §TR3 end-to-end (c13): drive a plugin `tool_result`
//! end-to-end so `handle_tool_result` records the result id in
//! `ReferencedTaintIndex.by_result_id` with the unioned canonical taint;
//! then a subsequent provider `tool_request` citing that result id
//! emerges with the unioned ancestry in its canonical taint — without
//! any manual `record_result` seed (c12's setup).

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
    await_topic, build_rig, subscribe_core_test_receiver, RigOpts, MOCK_PROVIDER_ID,
    READFILE_CANONICAL, READFILE_TOPIC_ID,
};

#[tokio::test]
async fn reemit_tool_request_unions_referenced_ancestry_end_to_end() {
    let rig = build_rig(RigOpts {
        include_readfile_plugin: true,
        tool_routes: vec![("readfile", READFILE_CANONICAL)],
        ..Default::default()
    });
    let plugin_canonical = rig.readfile_canonical.clone().expect("readfile present");

    let taint_match = Arc::new(TaintMatchMap::new(Duration::from_secs(300), 16));
    let referenced = Arc::new(ReferencedTaintIndex::new(Duration::from_secs(300)));

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

    // Seed a prior result id so the first provider tool_request can
    // satisfy the broker's `in_reply_to` cardinality requirement.
    let prior = JsonRpcId::from("prior-0");
    rig.broker
        .seed_provider_observed_result_for_test(&rig.provider_canonical, prior.clone());

    // Step 1: provider tool_request → handle_tool_request records
    // by_request_id with `[provider:mock]`.
    let req1 = JsonRpcId::from("req-1");
    let req1_params = serde_json::json!({
        "topic": format!("provider.{MOCK_PROVIDER_ID}.tool_request"),
        "payload": {"tool": "readfile", "args": {"path": "src/main.rs"}},
        "in_reply_to": [prior],
        "request_id": req1.clone(),
    });
    rig.broker
        .handle_provider_publish(&rig.provider_canonical, &req1_params)
        .expect("first tool_request accepted");
    let mut seen = Vec::new();
    let _ = await_topic(&mut canonical_rx, "core.session.tool_request", &mut seen).await;

    // Seed outstanding_dispatched manually (m5a gate path is bypassed
    // in this re-emit unit test) so `handle_plugin_publish` accepts the
    // tool_result whose `in_reply_to[0]` cites `req1`.
    rig.broker
        .publish_for_tool_dispatch(
            &plugin_canonical,
            serde_json::json!({}),
            req1.clone(),
            None,
            None,
            Vec::new(),
        )
        .expect("dispatch seeds outstanding map");

    // Step 2: plugin tool_result citing req1 →
    // handle_tool_result records by_result_id with the unioned taint
    // `[provider:mock, tool:readfile_canonical]`.
    let res1 = JsonRpcId::from("res-1");
    let res1_params = serde_json::json!({
        "topic": format!("plugin.{READFILE_TOPIC_ID}.tool_result"),
        "payload": {"content": "ok"},
        "in_reply_to": [req1],
        "request_id": res1.clone(),
    });
    rig.broker
        .handle_plugin_publish(&plugin_canonical, &res1_params)
        .expect("tool_result accepted");
    let _ = await_topic(&mut canonical_rx, "core.session.tool_result", &mut seen).await;

    // Seed provider_observed_results so the second provider tool_request
    // is allowed to cite `res1` in `in_reply_to`.
    rig.broker
        .seed_provider_observed_result_for_test(&rig.provider_canonical, res1.clone());

    // Step 3: second provider tool_request citing res1 →
    // handle_tool_request unions `lookup_result(res1)` into the
    // canonical taint, without any manual cache seed.
    let req2 = JsonRpcId::from("req-2");
    let req2_params = serde_json::json!({
        "topic": format!("provider.{MOCK_PROVIDER_ID}.tool_request"),
        "payload": {"tool": "readfile", "args": {"path": "src/lib.rs"}},
        "in_reply_to": [res1],
        "request_id": req2.clone(),
    });
    rig.broker
        .handle_provider_publish(&rig.provider_canonical, &req2_params)
        .expect("second tool_request accepted");
    let canonical = await_topic(&mut canonical_rx, "core.session.tool_request", &mut seen).await;
    assert_eq!(canonical.request_id.as_ref(), Some(&req2));

    let taint = canonical.taint.as_ref().expect("taint present");
    let provider_entry = TaintEntry {
        source: "provider".to_string(),
        detail: Some(MOCK_PROVIDER_ID.to_string()),
    };
    let tool_entry = TaintEntry {
        source: "tool".to_string(),
        detail: Some(READFILE_CANONICAL.to_string()),
    };
    assert!(
        taint.contains(&provider_entry),
        "provider entry present: {taint:?}",
    );
    assert!(
        taint.contains(&tool_entry),
        "tool entry from referenced-result ancestry present: {taint:?}",
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
