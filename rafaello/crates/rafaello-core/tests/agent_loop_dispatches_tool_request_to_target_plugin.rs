//! c19 / scope §AL5 (dispatch half): on `core.session.tool_request`,
//! the agent loop synthesises a per-plugin
//! `plugin.<topic-id>.tool_request` carrying the same `request_id`,
//! `in_reply_to`, and `taint` envelope, with `dispatch_target`
//! stripped from the inner payload.

use std::time::Duration;

use rafaello_core::agent::AgentLoop;
use rafaello_core::bus::{JsonRpcId, TaintEntry};
use tokio::sync::watch;

mod common;
use common::agent_test_kit::{
    build_agent_rig, AgentRigOpts, READFILE_CANONICAL, READFILE_TOPIC_ID,
};

#[tokio::test(flavor = "multi_thread")]
async fn agent_loop_dispatches_tool_request_to_target_plugin() {
    let rig = build_agent_rig(AgentRigOpts::default());

    let dispatch_topic = format!("plugin.{READFILE_TOPIC_ID}.tool_request");
    let (mut dispatch_rx, _dsub) = rig
        .broker
        .subscribe_internal(vec![dispatch_topic.clone()], 16);

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let agent = AgentLoop::new(
        rig.broker.clone(),
        rig.acl.clone(),
        rig.controller.clone(),
        rig.caps.clone(),
        shutdown_rx,
    );
    let join = agent.start();

    let request_id = JsonRpcId::from("req-42");
    let prior_user = JsonRpcId::from("um-1");
    rig.broker
        .publish_core_with_taint(
            "core.session.tool_request",
            serde_json::json!({
                "tool": "read-file",
                "args": {"path": "src/main.rs"},
                "dispatch_target": READFILE_CANONICAL,
            }),
            Some(request_id.clone()),
            Some(vec![prior_user.clone()]),
            Some(vec![TaintEntry {
                source: "provider".to_string(),
                detail: Some("mock".to_string()),
            }]),
            None,
        )
        .expect("publish accepted");

    let event = tokio::time::timeout(Duration::from_secs(2), dispatch_rx.recv())
        .await
        .expect("dispatch arrives within 2s")
        .expect("dispatch channel open");

    assert_eq!(event.topic, dispatch_topic);
    assert_eq!(event.request_id, Some(request_id));
    assert_eq!(event.in_reply_to.as_deref(), Some(&[prior_user][..]));
    assert_eq!(event.payload["tool"], "read-file");
    assert_eq!(event.payload["args"]["path"], "src/main.rs");
    assert!(
        event.payload.get("dispatch_target").is_none(),
        "dispatch_target stripped from inner payload"
    );
    let taint = event.taint.as_ref().expect("taint forwarded");
    assert_eq!(taint.len(), 1);
    assert_eq!(taint[0].source, "provider");
    assert_eq!(taint[0].detail.as_deref(), Some("mock"));

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("agent exits")
        .expect("agent task did not panic");
}
