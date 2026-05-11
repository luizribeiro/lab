//! c38 / scope §CG6 + pi-1 B-3 — agent-loop pivot.
//!
//! With **no `ConfirmationGate` constructed**, the agent loop must
//! observe `core.session.tool_request`, persist the `tool_call`
//! entry, and emit **no** `plugin.<topic-id>.tool_request`. This is
//! the unsplittable-cutover half that asserts the dispatch code path
//! is gone from the agent loop; the gate's construction in
//! `rafaello::run_chat` is the other half.

use std::time::Duration;

use rafaello_core::agent::AgentLoop;
use rafaello_core::bus::{JsonRpcId, TaintEntry};
use tokio::sync::watch;

mod common;
use common::agent_test_kit::{
    await_finalized, build_agent_rig, subscribe_finalized, AgentRigOpts, READFILE_CANONICAL,
    READFILE_TOPIC_ID,
};

#[tokio::test(flavor = "multi_thread")]
async fn agent_loop_does_not_dispatch_tool_request_directly() {
    let rig = build_agent_rig(AgentRigOpts::default());

    let dispatch_topic = format!("plugin.{READFILE_TOPIC_ID}.tool_request");
    let (mut dispatch_rx, _dsub) = rig.broker.subscribe_internal(vec![dispatch_topic], 16);
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

    rig.broker
        .publish_core_with_taint(
            "core.session.tool_request",
            serde_json::json!({
                "tool": "read-file",
                "args": {"path": "src/main.rs"},
                "dispatch_target": READFILE_CANONICAL,
            }),
            Some(JsonRpcId::from("req-pivot")),
            None,
            Some(vec![TaintEntry {
                source: "provider".to_string(),
                detail: Some("mock".to_string()),
            }]),
            None,
        )
        .expect("publish accepted");

    // The agent loop still persists the tool_call entry.
    let _ = await_finalized(&mut finalized_rx).await;

    // But with no gate constructed, no plugin.<topic>.tool_request
    // is ever published — the dispatch code path is gone.
    let outcome = tokio::time::timeout(Duration::from_millis(250), dispatch_rx.recv()).await;
    assert!(
        outcome.is_err(),
        "agent loop must not dispatch to plugin.<topic-id>.tool_request \
         after the c38 pivot; observed: {outcome:?}"
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("agent exits")
        .expect("agent task did not panic");
}
