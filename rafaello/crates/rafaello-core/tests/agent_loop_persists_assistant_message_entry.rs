//! c19 / scope §AL4: on `core.session.assistant_message`, the agent
//! loop persists a typed `text` entry with `author = Assistant`.

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
async fn agent_loop_persists_assistant_message_entry() {
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

    let prior_user = JsonRpcId::from("um-1");
    rig.broker
        .publish_core_with_taint(
            "core.session.assistant_message",
            serde_json::json!({"text": "hi there"}),
            Some(JsonRpcId::from("am-1")),
            Some(vec![prior_user]),
            Some(vec![TaintEntry {
                source: "provider".to_string(),
                detail: Some("mock".to_string()),
            }]),
            None,
        )
        .expect("publish accepted");

    let _ = await_finalized(&mut finalized_rx).await;

    let stored = load_single_entry(&rig.controller);
    assert_eq!(stored.entry.kind, "text");
    assert_eq!(stored.entry.metadata.author, EntryAuthor::Assistant);
    assert_eq!(stored.entry.payload["text"], "hi there");
    assert_eq!(stored.entry.payload["markdown"], false);

    shutdown_tx.send(true).expect("shutdown");
    tokio::time::timeout(Duration::from_secs(2), join)
        .await
        .expect("agent exits")
        .expect("agent task did not panic");
}
