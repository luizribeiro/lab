//! c14 / §PT1 — the synthetic deny `core.session.tool_result` emitted
//! on a superset violation flows through the m4 `tool_result`
//! persistence path. The persisted entries row carries `ok = false`,
//! `call_id = <originating tool_request request_id>`, and an empty
//! `content` string. `details` is `None` (round-2 cut, pi-4 B-2/B-3 —
//! the live agent loop drops the wire payload's `error` field at
//! persistence).

#![cfg(feature = "test-fixture")]

use std::time::Duration;

use rafaello_core::agent::AgentLoop;
use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::entry::EntryAuthor;
use tokio::sync::watch;

mod common;
use common::agent_test_kit::{
    await_finalized, build_agent_rig, load_single_entry, subscribe_finalized, AgentRigOpts,
};
use common::peer_test_kit::fresh_peer;

#[tokio::test(flavor = "multi_thread")]
async fn synthetic_result_persists_as_deny_tool_result_entry() {
    let rig = build_agent_rig(AgentRigOpts::default());
    let (mut finalized_rx, _fsub) = subscribe_finalized(&rig.broker);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let agent = AgentLoop::new(
        rig.broker.clone(),
        rig.acl.clone(),
        rig.controller.clone(),
        rig.caps.clone(),
        shutdown_rx,
    );
    let join = agent.start();

    let (peer, _rx) = fresh_peer();
    let _guard = rig
        .broker
        .register_plugin(rig.readfile_canonical.clone(), peer)
        .expect("readfile registered");

    let dispatch_id = JsonRpcId::from("req-c14i");
    let dispatch_taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some("rafaello-fetch".to_string()),
    }];
    rig.broker
        .publish_for_tool_dispatch(
            &rig.readfile_canonical,
            serde_json::json!({}),
            dispatch_id.clone(),
            None,
            None,
            dispatch_taint,
        )
        .expect("dispatch ok");

    let topic = format!(
        "plugin.{}.tool_result",
        common::agent_test_kit::READFILE_TOPIC_ID
    );
    let params = serde_json::json!({
        "topic": topic,
        "payload": {"ok": true, "content": "leaked"},
        "in_reply_to": [dispatch_id.clone()],
        "request_id": JsonRpcId::from("resp-c14i"),
        "taint": [{"source": "user", "detail": null}],
    });
    let _ = rig
        .broker
        .handle_plugin_publish(&rig.readfile_canonical, &params);

    let _ = await_finalized(&mut finalized_rx).await;

    let stored = load_single_entry(&rig.controller);
    assert_eq!(stored.entry.kind, "tool_result");
    assert_eq!(stored.entry.metadata.author, EntryAuthor::Tool);
    assert_eq!(stored.entry.payload["ok"], false);
    assert_eq!(stored.entry.payload["call_id"], dispatch_id.to_string());
    assert_eq!(stored.entry.payload["content"]["code"], "");
    assert!(
        stored.entry.payload.get("details").is_none(),
        "details remain None per round-2 cut / pi-4 B-2"
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("agent exits")
        .expect("agent task did not panic");
}
