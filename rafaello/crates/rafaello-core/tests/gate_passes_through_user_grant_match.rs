//! c20 — a `tool_request` whose target has sinks BUT a matching
//! `UserGrant` is dispatched through; audit row `gate_grant_match`
//! recorded.

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::Utc;
use parking_lot::RwLock;
use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId, TaintEntry};
use rafaello_core::compile::{
    CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan, ToolMeta,
};
use rafaello_core::gate::{ConfirmState, ConfirmationGate};
use rafaello_core::lock::canonical_id::CanonicalId;
use rafaello_core::lock::load_policy::LoadPolicy;
use rafaello_core::renderer::{RenderPipeline, RendererRegistry};
use rafaello_core::session::{SessionController, SessionStore};
use rafaello_core::user_grants::{GrantMatcher, GrantSource, UserGrant, UserGrants};
use ulid::Ulid;

mod common;
use common::peer_test_kit::fresh_peer;

#[tokio::test(flavor = "current_thread")]
async fn gate_passes_through_user_grant_match() {
    let target = CanonicalId::parse("local/test:mailer@0.1.0").expect("canonical");
    let target_topic_id = "mailer_local_test".to_string();
    let dispatch_topic = format!("plugin.{target_topic_id}.tool_request");

    let mut plugins = BTreeMap::new();
    plugins.insert(
        target.clone(),
        PluginAcl {
            topic_id: target_topic_id.clone(),
            publish_topics: vec![format!("plugin.{target_topic_id}.tool_result")],
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
    let (peer, mut peer_rx) = fresh_peer();
    let _guard = broker
        .register_plugin(target.clone(), peer)
        .expect("registration succeeds");

    let mut compiled = BTreeMap::new();
    compiled.insert(
        target.clone(),
        CompiledPlugin {
            canonical: target.clone(),
            topic_id: target_topic_id.clone(),
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

    let mut grants = UserGrants::new();
    grants.add(UserGrant {
        tool: "send_mail".to_string(),
        plugin: target.clone(),
        matcher: GrantMatcher::Any,
        added_at: Utc::now(),
        source: GrantSource::SlashCommand,
    });

    let tmp = tempfile::tempdir().expect("state tempdir");
    let store = SessionStore::open(tmp.path()).expect("session store opens");
    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let controller = SessionController::new(store, pipeline, broker.clone());
    let audit = controller.audit_writer();

    let gate = ConfirmationGate::new(
        Arc::new(broker.clone()),
        Arc::new(RwLock::new(grants)),
        Arc::clone(&audit),
        Arc::new(ConfirmState::new()),
        compiled,
        std::sync::Arc::new(rafaello_core::supervisor::PluginSupervisor::new(
            broker.clone(),
            rafaello_core::supervisor::SupervisorConfig::default(),
            std::sync::Arc::new(
                rafaello_core::supervisor::ToolSchemaCatalog::build(
                    &rafaello_core::broker_acl::BrokerAcl::default(),
                    &std::collections::BTreeMap::new(),
                    &std::collections::BTreeMap::new(),
                )
                .expect("empty catalog"),
            ),
        )),
    );
    let _handle = gate.spawn();

    let request_id = JsonRpcId::from(Ulid::new().to_string());
    broker
        .publish_core_with_taint(
            "core.session.tool_request",
            serde_json::json!({
                "tool": "send_mail",
                "args": {"to": "a@b.c"},
                "dispatch_target": target.to_string(),
            }),
            Some(request_id.clone()),
            None,
            Some(vec![TaintEntry {
                source: "user".to_string(),
                detail: None,
            }]),
            None,
        )
        .expect("publish tool_request");

    let notification = tokio::time::timeout(std::time::Duration::from_secs(1), async move {
        loop {
            if let Some(n) = peer_rx.recv().await {
                return n;
            }
        }
    })
    .await
    .expect("peer receives dispatch within timeout");
    assert_eq!(
        notification.params["topic"],
        serde_json::json!(dispatch_topic)
    );
    assert_eq!(
        notification.params["payload"]["tool"],
        serde_json::json!("send_mail")
    );

    let conn =
        rusqlite::Connection::open(tmp.path().join("session.sqlite")).expect("readback connection");
    let kind: String = conn
        .query_row(
            "SELECT kind FROM audit_events WHERE request_id = ?1",
            [request_id.to_string()],
            |row| row.get(0),
        )
        .expect("audit row present");
    assert_eq!(kind, "gate_grant_match");
}
