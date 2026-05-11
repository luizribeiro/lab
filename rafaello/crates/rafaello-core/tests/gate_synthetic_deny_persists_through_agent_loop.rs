//! c22 §CG4a: the synthetic deny `tool_result` produced by
//! `synthesise_deny_tool_result` round-trips cleanly through
//! `agent/mod.rs::handle_tool_result` — a persisted `tool_result`
//! entry with `ok: false` and `call_id` pinned to the held
//! tool_request id.

use std::time::{Duration, Instant};

use rafaello_core::agent::AgentLoop;
use rafaello_core::bus::{BusEvent, JsonRpcId, PublisherIdentity, TaintEntry};
use rafaello_core::entry::EntryAuthor;
use rafaello_core::gate::{synthesise_deny_tool_result, DenyReason, HeldConfirmation};
use rafaello_core::lock::canonical_id::CanonicalId;
use tokio::sync::watch;
use ulid::Ulid;

mod common;
use common::agent_test_kit::{
    await_finalized, build_agent_rig, load_single_entry, subscribe_finalized, AgentRigOpts,
};

#[tokio::test(flavor = "multi_thread")]
async fn gate_synthetic_deny_persists_through_agent_loop() {
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

    let tool_request_id = JsonRpcId::from(Ulid::new().to_string());
    let held = HeldConfirmation {
        tool_request: BusEvent {
            topic: "core.session.tool_request".into(),
            payload: serde_json::json!({"tool": "send_mail", "args": {"to": "x"}}),
            publisher: PublisherIdentity::Core,
            in_reply_to: None,
            taint: Some(vec![TaintEntry {
                source: "user".to_string(),
                detail: None,
            }]),
            request_id: Some(tool_request_id.clone()),
        },
        deadline: Instant::now() + Duration::from_secs(60),
        dispatch_target: CanonicalId::parse("local/test:mailer@0.1.0").unwrap(),
    };

    let args = synthesise_deny_tool_result(&held, DenyReason::UserDenied);
    rig.broker
        .publish_core_with_taint(
            args.topic,
            args.payload,
            args.request_id,
            args.in_reply_to,
            args.taint,
            None,
        )
        .expect("synthetic deny publishes through m4 envelope rules");

    let _ = await_finalized(&mut finalized_rx).await;

    let stored = load_single_entry(&rig.controller);
    assert_eq!(stored.entry.kind, "tool_result");
    assert_eq!(stored.entry.metadata.author, EntryAuthor::Tool);
    assert_eq!(stored.entry.payload["ok"], false);
    assert_eq!(stored.entry.payload["call_id"], tool_request_id.to_string(),);

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("agent exits")
        .expect("agent task did not panic");
}
