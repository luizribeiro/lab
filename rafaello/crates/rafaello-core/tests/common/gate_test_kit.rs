#![allow(dead_code)]
//! Shared rig for c22 CG4 integration tests.
//!
//! Builds a `ConfirmationGate` over an in-memory broker + tempdir
//! session store, with a single `mailer` plugin whose `send_mail`
//! tool declares `sinks = ["mail"]` so the gate's hold path is
//! exercised when needed. Tests can also `state.reserve` directly
//! to bypass the hold path and drive CG4 in isolation.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use rafaello_core::audit::AuditWriter;
use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, BusEvent, JsonRpcId, PublisherIdentity, TaintEntry};
use rafaello_core::compile::{
    CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan, ToolMeta,
};
use rafaello_core::gate::{ConfirmState, ConfirmationGate, HeldConfirmation};
use rafaello_core::lock::canonical_id::CanonicalId;
use rafaello_core::lock::load_policy::LoadPolicy;
use rafaello_core::renderer::{RenderPipeline, RendererRegistry};
use rafaello_core::session::{SessionController, SessionStore};
use rafaello_core::supervisor::{PluginSupervisor, SupervisorConfig, ToolSchemaCatalog};
use rafaello_core::user_grants::UserGrants;
use tempfile::TempDir;
use tokio::task::JoinHandle;
use ulid::Ulid;

use fittings_core::context::OutboundNotification;
use tokio::sync::mpsc;

use super::peer_test_kit::fresh_peer;

pub const MAILER_CANONICAL: &str = "local/test:mailer@0.1.0";
pub const MAILER_TOPIC_ID: &str = "mailer_local_test";

pub struct GateRig {
    pub broker: Broker,
    pub state: Arc<ConfirmState>,
    pub user_grants: Arc<RwLock<UserGrants>>,
    pub audit: Arc<AuditWriter>,
    pub gate_handle: JoinHandle<()>,
    pub target: CanonicalId,
    pub peer_rx: mpsc::Receiver<OutboundNotification>,
    pub tmp: TempDir,
}

pub fn build_gate_rig() -> GateRig {
    let target = CanonicalId::parse(MAILER_CANONICAL).expect("canonical");
    let dispatch_topic = format!("plugin.{MAILER_TOPIC_ID}.tool_request");

    let mut plugins = BTreeMap::new();
    plugins.insert(
        target.clone(),
        PluginAcl {
            topic_id: MAILER_TOPIC_ID.to_string(),
            publish_topics: vec![format!("plugin.{MAILER_TOPIC_ID}.tool_result")],
            subscribe_patterns: vec![],
            auto_subscribes: vec![dispatch_topic.clone()],
            provider_id: None,
        },
    );
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl is well-formed");
    let (peer, peer_rx) = fresh_peer();
    std::mem::forget(
        broker
            .register_plugin(target.clone(), peer)
            .expect("registration succeeds"),
    );

    let mut compiled = BTreeMap::new();
    compiled.insert(
        target.clone(),
        CompiledPlugin {
            canonical: target.clone(),
            topic_id: MAILER_TOPIC_ID.to_string(),
            entry_absolute: std::path::PathBuf::from("/dev/null"),
            filesystem: FilesystemPlan::default(),
            network: NetworkPlan::Deny,
            env: EnvPlan::default(),
            limits: LimitsPlan::default(),
            subscribe_patterns: vec![],
            publish_topics: vec![],
            auto_subscribes: vec![dispatch_topic.clone()],
            tool_meta: BTreeMap::from([(
                "send_mail".to_string(),
                ToolMeta {
                    sinks: vec!["mail".to_string()],
                    sinks_inferred: false,
                    grant_match: None,
                    always_confirm: false,
                },
            )]),
            provider_id: None,
            load: LoadPolicy::Manual,
            flags: CompiledFlags::default(),
        },
    );

    let tmp = tempfile::tempdir().expect("state tempdir");
    let store = SessionStore::open(tmp.path()).expect("session store opens");
    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let controller = SessionController::new(store, pipeline, broker.clone());
    let audit = controller.audit_writer();
    let state = Arc::new(ConfirmState::new());
    let user_grants = Arc::new(RwLock::new(UserGrants::new()));

    let supervisor = Arc::new(PluginSupervisor::new(
        broker.clone(),
        SupervisorConfig::default(),
        Arc::new(
            ToolSchemaCatalog::build(&BrokerAcl::default(), &BTreeMap::new(), &BTreeMap::new())
                .expect("empty catalog builds"),
        ),
    ));
    let gate = ConfirmationGate::new(
        Arc::new(broker.clone()),
        Arc::clone(&user_grants),
        Arc::clone(&audit),
        Arc::clone(&state),
        compiled,
        supervisor,
    );
    let gate_handle = gate.spawn();

    GateRig {
        broker,
        state,
        user_grants,
        audit,
        gate_handle,
        target,
        peer_rx,
        tmp,
    }
}

/// Seed a held confirmation directly in the rig's `ConfirmState`.
/// Returns `(confirm_id, tool_request_id)`. The `BusEvent` carried
/// in the held entry has the full `core.session.tool_request`
/// payload shape (`tool`, `args`, `dispatch_target`).
pub fn seed_held(rig: &GateRig, tool: &str, args: serde_json::Value) -> (JsonRpcId, JsonRpcId) {
    let confirm_id = JsonRpcId::from(Ulid::new().to_string());
    let tool_request_id = JsonRpcId::from(Ulid::new().to_string());
    let tool_request = BusEvent {
        topic: "core.session.tool_request".into(),
        payload: serde_json::json!({
            "tool": tool,
            "args": args,
            "dispatch_target": rig.target.to_string(),
        }),
        publisher: PublisherIdentity::Core,
        in_reply_to: None,
        taint: Some(vec![TaintEntry {
            source: "user".to_string(),
            detail: None,
        }]),
        request_id: Some(tool_request_id.clone()),
    };
    rig.state.reserve(
        confirm_id.clone(),
        HeldConfirmation {
            tool_request,
            deadline: Instant::now() + Duration::from_secs(60),
            dispatch_target: rig.target.clone(),
        },
    );
    (confirm_id, tool_request_id)
}

pub fn publish_confirm_reply(broker: &Broker, confirm_id: &JsonRpcId, answer: &str) {
    broker
        .publish_core_with_taint(
            "core.session.confirm_reply",
            serde_json::json!({
                "request_id": confirm_id.to_string(),
                "answer": answer,
            }),
            Some(JsonRpcId::from(Ulid::new().to_string())),
            Some(vec![confirm_id.clone()]),
            Some(vec![TaintEntry {
                source: "user".to_string(),
                detail: None,
            }]),
            None,
        )
        .expect("publish confirm_reply");
}

pub fn audit_kinds(rig: &GateRig, request_id: &JsonRpcId) -> Vec<String> {
    let conn = rusqlite::Connection::open(rig.tmp.path().join("session.sqlite"))
        .expect("readback connection");
    let mut stmt = conn
        .prepare("SELECT kind FROM audit_events WHERE request_id = ?1 ORDER BY seq")
        .expect("prepare");
    let rows = stmt
        .query_map([request_id.to_string()], |row| row.get::<_, String>(0))
        .expect("query");
    rows.filter_map(Result::ok).collect()
}
