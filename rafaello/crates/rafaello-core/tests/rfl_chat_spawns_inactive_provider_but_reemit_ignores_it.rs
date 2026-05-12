//! c27 / scope §C38b / m5a retro §5 item 13 — inactive-provider re-emit
//! is ignored by the router.
//!
//! With two providers in the BrokerAcl — active `builtin:openai@0.0.0` +
//! inactive `local:mockprovider@0.0.0` — and only `openai` selected as
//! the active provider, a fake `provider.mock.assistant_message`
//! published by the inactive provider MUST NOT be consumed by the
//! `ReemitRouter`: the router subscribes only to `provider.openai.**`,
//! so no `core.session.assistant_message` is fanned out and the agent
//! loop's persisted-entries delta stays at zero.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use rafaello_core::agent::AgentLoop;
use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId};
use rafaello_core::lock::CanonicalId;
use rafaello_core::reemit::ReemitRouter;
use rafaello_core::renderer::{Capabilities, RenderPipeline, RendererRegistry};
use rafaello_core::session::{SessionController, SessionStore};
use tokio::sync::watch;

mod common;
use common::peer_test_kit::fresh_peer;
use common::reemit_test_kit::drain_for;

const OPENAI_CANONICAL: &str = "builtin:openai@0.0.0";
const OPENAI_PROVIDER_ID: &str = "openai";
const OPENAI_TOPIC_ID: &str = "openai_builtin";
const MOCK_CANONICAL: &str = "local:mockprovider@0.0.0";
const MOCK_PROVIDER_ID: &str = "mock";
const MOCK_TOPIC_ID: &str = "mockprovider_local";

fn cid(s: &str) -> CanonicalId {
    CanonicalId::parse(s).expect("canonical id parses")
}

fn provider_acl(topic_id: &str, provider_id: &str) -> PluginAcl {
    PluginAcl {
        topic_id: topic_id.to_string(),
        publish_topics: vec![
            format!("provider.{provider_id}.tool_request"),
            format!("provider.{provider_id}.assistant_message"),
        ],
        subscribe_patterns: vec![],
        auto_subscribes: vec![],
        provider_id: Some(provider_id.to_string()),
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn inactive_provider_assistant_message_ignored_by_reemit_router() {
    let openai_canonical = cid(OPENAI_CANONICAL);
    let mock_canonical = cid(MOCK_CANONICAL);

    let mut plugins = BTreeMap::new();
    plugins.insert(
        openai_canonical.clone(),
        provider_acl(OPENAI_TOPIC_ID, OPENAI_PROVIDER_ID),
    );
    plugins.insert(
        mock_canonical.clone(),
        provider_acl(MOCK_TOPIC_ID, MOCK_PROVIDER_ID),
    );

    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl.clone()).expect("acl well-formed");

    let (openai_peer, _openai_rx) = fresh_peer();
    let _openai_guard = broker
        .register_provider(openai_canonical.clone(), openai_peer)
        .expect("openai registers");
    let (mock_peer, _mock_rx) = fresh_peer();
    let _mock_guard = broker
        .register_provider(mock_canonical.clone(), mock_peer)
        .expect("mock registers");

    let tmp = tempfile::tempdir().expect("state tempdir");
    let store = SessionStore::open(tmp.path()).expect("session store opens");
    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let controller = Arc::new(SessionController::new(store, pipeline, broker.clone()));

    let (router_shutdown_tx, router_shutdown_rx) = watch::channel(false);
    let router = ReemitRouter::new(
        broker.clone(),
        acl.clone(),
        openai_canonical.clone(),
        router_shutdown_rx,
    );
    let router_join = router.start();

    let (agent_shutdown_tx, agent_shutdown_rx) = watch::channel(false);
    let agent = AgentLoop::new(
        broker.clone(),
        acl.clone(),
        controller.clone(),
        Capabilities::tui_default(),
        agent_shutdown_rx,
    );
    let agent_join = agent.start();

    let (mut core_rx, _csub) = broker.subscribe_internal(vec!["core.session.**".to_string()], 32);

    let seed_id = JsonRpcId::from("user-msg-seed-c27");
    broker.seed_provider_observed_user_message_for_test(&mock_canonical, seed_id.clone());

    let params = serde_json::json!({
        "topic": format!("provider.{MOCK_PROVIDER_ID}.assistant_message"),
        "payload": {"text": "i should be invisible"},
        "in_reply_to": [seed_id.clone()],
        "request_id": JsonRpcId::from("asst-c27"),
    });
    broker
        .handle_provider_publish(&mock_canonical, &params)
        .expect("broker accepts the inactive provider's publish");

    let observed = drain_for(&mut core_rx, Duration::from_millis(250)).await;
    let assistant_topics: Vec<&str> = observed
        .iter()
        .map(|e| e.topic.as_str())
        .filter(|t| *t == "core.session.assistant_message")
        .collect();
    assert!(
        assistant_topics.is_empty(),
        "ReemitRouter must stay scoped to provider.openai.** — observed \
         core.session.assistant_message for the inactive mock provider; \
         core.session.* events seen: {:?}",
        observed.iter().map(|e| &e.topic).collect::<Vec<_>>()
    );

    let entries = controller.store().load_entries().expect("load_entries");
    assert!(
        entries.is_empty(),
        "agent loop persisted-entries delta must be zero for an inactive \
         provider's assistant_message; persisted entries: {entries:?}"
    );

    agent_shutdown_tx.send(true).expect("agent shutdown");
    router_shutdown_tx.send(true).expect("router shutdown");
    tokio::time::timeout(Duration::from_secs(2), agent_join)
        .await
        .expect("agent loop exits")
        .expect("agent loop did not panic");
    tokio::time::timeout(Duration::from_secs(2), router_join)
        .await
        .expect("router exits")
        .expect("router did not panic");
}
