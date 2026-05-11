//! c38 / pi-1 B-3 — no double dispatch.
//!
//! With both the agent loop and the confirmation gate subscribed to
//! `core.session.tool_request`, exactly one
//! `plugin.<topic-id>.tool_request` must be published per inbound
//! tool_request. This guards against the regression that motivated
//! the unsplittable cutover: if the agent loop continued dispatching
//! after c38 wired the gate, every request would dispatch twice.
//!
//! Placement note: row spec lists this under `rafaello/tests/` but
//! the assertion is structurally a broker/gate/agent-loop unit test
//! that does not require a real five-tree process; placing it in
//! `rafaello-core/tests/` co-locates it with the gate_test_kit.

use std::time::Duration;

use rafaello_core::agent::AgentLoop;
use rafaello_core::bus::{JsonRpcId, TaintEntry};
use tokio::sync::watch;

mod common;
use common::gate_test_kit::{build_gate_rig, MAILER_TOPIC_ID};

#[tokio::test(flavor = "multi_thread")]
async fn rfl_chat_no_double_dispatch_when_gate_constructed() {
    let rig = build_gate_rig();
    let dispatch_topic = format!("plugin.{MAILER_TOPIC_ID}.tool_request");
    let (mut dispatch_rx, _dsub) = rig
        .broker
        .subscribe_internal(vec![dispatch_topic.clone()], 16);

    // Construct the rafaello-core session machinery the agent loop
    // needs. We reuse the gate rig's broker; the gate is already
    // running. Build a fresh session controller for the agent loop.
    let store_dir = tempfile::tempdir().unwrap();
    let store = rafaello_core::session::SessionStore::open(store_dir.path()).unwrap();
    let pipeline = rafaello_core::renderer::RenderPipeline::new(std::sync::Arc::new(
        rafaello_core::renderer::RendererRegistry::with_builtins(),
    ));
    let controller = std::sync::Arc::new(rafaello_core::session::SessionController::new(
        store,
        pipeline,
        rig.broker.clone(),
    ));
    let caps = rafaello_core::renderer::Capabilities::tui_default();

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let agent = AgentLoop::new(
        rig.broker.clone(),
        rafaello_core::broker_acl::BrokerAcl {
            plugins: std::collections::BTreeMap::new(),
            tool_routes: std::collections::BTreeMap::new(),
            frontends: std::collections::BTreeMap::new(),
        },
        controller,
        caps,
        shutdown_rx,
    );
    let agent_join = agent.start();

    // Drive one tool_request. The mailer plugin's `send_mail` tool
    // declares `sinks = ["mail"]`, so the gate would normally HOLD;
    // we want passthrough here so the gate dispatches without
    // confirmation. Use a tool with no sinks — `send_mail_anon`
    // isn't in the catalog, so use a non-sink call by feeding a
    // grant. Simpler path: pre-load a `Any` grant for `send_mail`.
    use rafaello_core::user_grants::{GrantMatcher, GrantSource, UserGrant};
    rig.user_grants.write().add(UserGrant {
        tool: "send_mail".to_string(),
        plugin: rig.target.clone(),
        matcher: GrantMatcher::Any,
        added_at: chrono::Utc::now(),
        source: GrantSource::SlashCommand,
    });

    let request_id = JsonRpcId::from("req-c38");
    rig.broker
        .publish_core_with_taint(
            "core.session.tool_request",
            serde_json::json!({
                "tool": "send_mail",
                "args": {"to": "alice@example.com"},
                "dispatch_target": rig.target.to_string(),
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

    // Collect every dispatch for 300 ms.
    let mut dispatch_count = 0usize;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
    while let Ok(Some(event)) = tokio::time::timeout_at(deadline, dispatch_rx.recv()).await {
        if event.topic == dispatch_topic {
            dispatch_count += 1;
        }
    }
    assert_eq!(
        dispatch_count, 1,
        "exactly one plugin.<topic>.tool_request must be published; got {dispatch_count}"
    );

    shutdown_tx.send(true).expect("shutdown");
    let _ = tokio::time::timeout(Duration::from_secs(2), agent_join).await;
    rig.gate_handle.abort();
    let _ = rig.gate_handle.await;
}
