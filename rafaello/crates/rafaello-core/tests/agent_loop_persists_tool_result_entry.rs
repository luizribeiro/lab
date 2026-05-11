//! c19 / scope §AL6: on `core.session.tool_result`, the agent loop
//! persists a typed `tool_result` entry with `author = Tool`, `ok`
//! forwarded, the wire `content` string wrapped as
//! `RenderNode::Code { code, lang: None }`, and `call_id` set to
//! `in_reply_to[0]` rendered as a string. `details` remain `None`
//! (round-2 cut).

use std::time::Duration;

use rafaello_core::agent::AgentLoop;
use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::entry::EntryAuthor;
use tokio::sync::watch;

mod common;
use common::agent_test_kit::{
    await_finalized, build_agent_rig, load_single_entry, subscribe_finalized, AgentRigOpts,
};

#[tokio::test(flavor = "multi_thread")]
async fn agent_loop_persists_tool_result_entry() {
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

    let call_id = JsonRpcId::from("req-9");
    rig.broker
        .publish_core_with_taint(
            "core.session.tool_result",
            serde_json::json!({"ok": true, "content": "fn main() {}"}),
            Some(JsonRpcId::from("res-9")),
            Some(vec![call_id.clone()]),
            Some(vec![TaintEntry {
                source: "tool".to_string(),
                detail: Some("local/test:readfile@0.1.0".to_string()),
            }]),
            None,
        )
        .expect("publish accepted");

    let _ = await_finalized(&mut finalized_rx).await;

    let stored = load_single_entry(&rig.controller);
    assert_eq!(stored.entry.kind, "tool_result");
    assert_eq!(stored.entry.metadata.author, EntryAuthor::Tool);
    assert_eq!(stored.entry.payload["call_id"], call_id.to_string());
    assert_eq!(stored.entry.payload["ok"], true);
    assert_eq!(stored.entry.payload["content"]["node"], "Code");
    assert_eq!(stored.entry.payload["content"]["code"], "fn main() {}");
    assert!(
        stored.entry.payload["content"].get("lang").is_none(),
        "lang elided via skip_serializing_if when None"
    );
    assert!(
        stored.entry.payload.get("details").is_none(),
        "details remain None (round-2 cut)"
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("agent exits")
        .expect("agent task did not panic");
}
