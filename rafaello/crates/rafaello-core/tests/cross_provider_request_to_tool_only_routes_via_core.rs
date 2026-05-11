//! c19 / pi-2 M2-2: even when a tool plugin's `bus.subscribes`
//! explicitly includes `core.session.tool_request`, the dispatch
//! through the agent loop publishes exactly one
//! `plugin.<topic-id>.tool_request` — the executable hop. The plugin
//! observes the canonical event as well (subscribe pattern matches)
//! but a tool implementation that only acts on the per-plugin topic
//! sees a single dispatch (so a single `tool_result` per request).

use std::time::Duration;

use rafaello_core::agent::AgentLoop;
use rafaello_core::bus::{JsonRpcId, TaintEntry};
use tokio::sync::watch;

mod common;
use common::agent_test_kit::{
    build_agent_rig, AgentRigOpts, READFILE_CANONICAL, READFILE_TOPIC_ID,
};
use common::peer_test_kit::fresh_peer;
use common::synthetic_dispatch;

#[tokio::test(flavor = "multi_thread")]
async fn cross_provider_request_to_tool_only_routes_via_core() {
    let rig = build_agent_rig(AgentRigOpts {
        readfile_extra_subscribes: vec!["core.session.tool_request".to_string()],
    });
    let dispatch_topic = format!("plugin.{READFILE_TOPIC_ID}.tool_request");

    let (peer, mut readfile_rx) = fresh_peer();
    let _guard = rig
        .broker
        .register_plugin(rig.readfile_canonical.clone(), peer)
        .expect("readfile plugin registers");

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let agent = AgentLoop::new(
        rig.broker.clone(),
        rig.acl.clone(),
        rig.controller.clone(),
        rig.caps.clone(),
        shutdown_rx,
    );
    let join = agent.start();
    // c38: agent loop no longer dispatches; m4 cross-provider routing
    // assertion exercised via the gate_or_synthetic_dispatch helper.
    let dispatcher = synthetic_dispatch::spawn(rig.broker.clone());

    let request_id = JsonRpcId::from("req-100");
    rig.broker
        .publish_core_with_taint(
            "core.session.tool_request",
            serde_json::json!({
                "tool": "read-file",
                "args": {"path": "src/lib.rs"},
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

    let mut canonical_seen = 0usize;
    let mut dispatch_seen = 0usize;
    let mut other_dispatch_seen = 0usize;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(500);
    while let Ok(Some(notification)) = tokio::time::timeout_at(deadline, readfile_rx.recv()).await {
        assert_eq!(notification.method, "bus.event");
        let topic = notification.params["topic"].as_str().unwrap_or("");
        if topic == "core.session.tool_request" {
            canonical_seen += 1;
        } else if topic == dispatch_topic {
            dispatch_seen += 1;
        } else if topic.starts_with("plugin.") && topic.ends_with(".tool_request") {
            other_dispatch_seen += 1;
        }
    }

    assert_eq!(
        canonical_seen, 1,
        "tool plugin observes the canonical event once (informational fan-out)"
    );
    assert_eq!(
        dispatch_seen, 1,
        "agent-loop publishes exactly one per-plugin dispatch"
    );
    assert_eq!(
        other_dispatch_seen, 0,
        "no spurious plugin.*.tool_request fan-out from the canonical event"
    );

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("agent exits")
        .expect("agent task did not panic");
    dispatcher.shutdown.send(true).expect("dispatcher shutdown");
    let _ = tokio::time::timeout(Duration::from_secs(2), dispatcher.join).await;
}
