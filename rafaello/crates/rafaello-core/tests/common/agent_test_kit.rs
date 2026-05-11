#![allow(dead_code)]
//! Shared helpers for c19 agent-loop tests.
//!
//! Builds a real `SessionController` over a tempdir SQLite store wired
//! to an in-memory `Broker`. The ACL contains a single mock provider
//! plugin + a readfile tool plugin so tests can drive
//! `core.session.tool_request` with a valid `dispatch_target` and
//! observe the `plugin.<topic-id>.tool_request` re-publish.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, BusEvent, InternalSubscription};
use rafaello_core::lock::CanonicalId;
use rafaello_core::renderer::{Capabilities, RenderPipeline, RendererRegistry};
use rafaello_core::session::{SessionController, SessionStore, StoredEntry};
use tempfile::TempDir;
use tokio::sync::mpsc;

pub const MOCK_PROVIDER_ID: &str = "mock";
pub const MOCK_TOPIC_ID: &str = "mockprov_local_test";
pub const MOCK_CANONICAL: &str = "local/test:mockprov@0.1.0";
pub const READFILE_TOPIC_ID: &str = "readfile_local_test";
pub const READFILE_CANONICAL: &str = "local/test:readfile@0.1.0";

pub struct AgentRig {
    pub broker: Broker,
    pub acl: BrokerAcl,
    pub controller: Arc<SessionController>,
    pub caps: Capabilities,
    pub provider_canonical: CanonicalId,
    pub readfile_canonical: CanonicalId,
    pub _tmp: TempDir,
}

pub fn cid(s: &str) -> CanonicalId {
    CanonicalId::parse(s).expect("canonical id parses")
}

#[derive(Default)]
pub struct AgentRigOpts {
    /// When set, the readfile plugin's `subscribe_patterns` includes
    /// the canonical core event named here. Used by the cross-provider
    /// routing test (pi-2 M2-2) to assert tool plugins that *do*
    /// subscribe to canonical events still only execute via the
    /// per-plugin dispatch hop.
    pub readfile_extra_subscribes: Vec<String>,
}

pub fn build_agent_rig(opts: AgentRigOpts) -> AgentRig {
    let provider_canonical = cid(MOCK_CANONICAL);
    let readfile_canonical = cid(READFILE_CANONICAL);

    let provider_acl = PluginAcl {
        topic_id: MOCK_TOPIC_ID.to_string(),
        publish_topics: vec![
            format!("provider.{MOCK_PROVIDER_ID}.tool_request"),
            format!("provider.{MOCK_PROVIDER_ID}.assistant_message"),
        ],
        subscribe_patterns: vec![],
        auto_subscribes: vec![],
        provider_id: Some(MOCK_PROVIDER_ID.to_string()),
    };
    let readfile_acl = PluginAcl {
        topic_id: READFILE_TOPIC_ID.to_string(),
        publish_topics: vec![format!("plugin.{READFILE_TOPIC_ID}.tool_result")],
        subscribe_patterns: opts.readfile_extra_subscribes,
        auto_subscribes: vec![format!("plugin.{READFILE_TOPIC_ID}.tool_request")],
        provider_id: None,
    };

    let mut plugins = BTreeMap::new();
    plugins.insert(provider_canonical.clone(), provider_acl);
    plugins.insert(readfile_canonical.clone(), readfile_acl);

    let mut tool_routes = BTreeMap::new();
    tool_routes.insert("read-file".to_string(), readfile_canonical.clone());

    let acl = BrokerAcl {
        plugins,
        tool_routes,
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl.clone()).expect("acl well-formed");

    let tmp = tempfile::tempdir().expect("state tempdir");
    let store = SessionStore::open(tmp.path()).expect("session store opens");
    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let controller = Arc::new(SessionController::new(store, pipeline, broker.clone()));

    AgentRig {
        broker,
        acl,
        controller,
        caps: Capabilities::tui_default(),
        provider_canonical,
        readfile_canonical,
        _tmp: tmp,
    }
}

/// Subscribe to `core.session.entry.finalized` — used as a sync barrier
/// for tests that drive a `core.session.*` event and need to wait for
/// the agent loop's persistence hop before reading SQLite.
pub fn subscribe_finalized(broker: &Broker) -> (mpsc::Receiver<BusEvent>, InternalSubscription) {
    broker.subscribe_internal(vec!["core.session.entry.finalized".to_string()], 16)
}

pub async fn await_finalized(rx: &mut mpsc::Receiver<BusEvent>) -> BusEvent {
    tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .expect("entry.finalized arrives within 2s")
        .expect("entry.finalized channel open")
}

pub fn load_single_entry(controller: &SessionController) -> StoredEntry {
    let mut entries = controller.store().load_entries().expect("load_entries");
    assert_eq!(entries.len(), 1, "exactly one persisted entry");
    entries.remove(0)
}
