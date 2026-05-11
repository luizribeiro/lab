//! c19 / scope §AL5 (persistence half): on `core.session.tool_request`,
//! the agent loop persists a typed `tool_call` entry with `author =
//! Assistant`, `status = Pending`, and `id` equal to the request_id
//! rendered as a string.

use std::time::Duration;

use rafaello_core::agent::AgentLoop;
use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::entry::EntryAuthor;
use tokio::sync::watch;

mod common;
use common::agent_test_kit::{
    await_finalized, build_agent_rig, load_single_entry, subscribe_finalized, AgentRigOpts,
    READFILE_CANONICAL,
};

#[tokio::test(flavor = "multi_thread")]
async fn agent_loop_persists_tool_call_entry() {
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

    let request_id = JsonRpcId::from("req-7");
    rig.broker
        .publish_core_with_taint(
            "core.session.tool_request",
            serde_json::json!({
                "tool": "read-file",
                "args": {"path": "src/main.rs"},
                "dispatch_target": READFILE_CANONICAL,
            }),
            Some(request_id.clone()),
            None,
            Some(vec![TaintEntry {
                source: "provider".to_string(),
                detail: Some("mock".to_string()),
            }]),
            None,
        )
        .expect("publish accepted");

    let _ = await_finalized(&mut finalized_rx).await;

    let stored = load_single_entry(&rig.controller);
    assert_eq!(stored.entry.kind, "tool_call");
    assert_eq!(stored.entry.metadata.author, EntryAuthor::Assistant);
    assert_eq!(stored.entry.payload["id"], request_id.to_string());
    assert_eq!(stored.entry.payload["name"], "read-file");
    assert_eq!(stored.entry.payload["status"], "pending");
    assert_eq!(stored.entry.payload["args"]["path"], "src/main.rs");

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("agent exits")
        .expect("agent task did not panic");
}
