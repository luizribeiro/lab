//! c04 pi-2 M-1 — the gate is the populator boundary for
//! `OutstandingDispatch.tool_request_taint`. Drive the three gate
//! dispatch arms that call `publish_for_tool_dispatch` (passthrough,
//! grant-match, post-confirm-allow) and assert the recorded entry
//! reflects the inbound canonical taint.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use parking_lot::RwLock;
use rafaello_core::audit::AuditWriter;
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
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use ulid::Ulid;

mod common;
use common::gate_test_kit::{build_gate_rig, publish_confirm_reply, seed_held, MAILER_CANONICAL};
use common::peer_test_kit::fresh_peer;

struct PassthroughRig {
    broker: Broker,
    target: CanonicalId,
    _gate_handle: JoinHandle<()>,
    _audit: Arc<AuditWriter>,
    _state: Arc<ConfirmState>,
    _user_grants: Arc<RwLock<UserGrants>>,
    _peer_rx: mpsc::Receiver<fittings_core::context::OutboundNotification>,
    _tmp: tempfile::TempDir,
}

fn build_passthrough_rig() -> PassthroughRig {
    let target = CanonicalId::parse("local/test:tool_plug@0.1.0").expect("canonical");
    let topic_id = "tool_plug_local_test".to_string();
    let dispatch_topic = format!("plugin.{topic_id}.tool_request");

    let mut plugins = BTreeMap::new();
    plugins.insert(
        target.clone(),
        PluginAcl {
            topic_id: topic_id.clone(),
            publish_topics: vec![format!("plugin.{topic_id}.tool_result")],
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
    let broker = Broker::new(acl).expect("acl well-formed");
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
            topic_id: topic_id.clone(),
            entry_absolute: std::path::PathBuf::from("/dev/null"),
            filesystem: FilesystemPlan::default(),
            network: NetworkPlan::Deny,
            env: EnvPlan::default(),
            limits: LimitsPlan::default(),
            subscribe_patterns: vec![],
            publish_topics: vec![],
            auto_subscribes: vec![dispatch_topic],
            tool_meta: BTreeMap::from([(
                "noop".to_string(),
                ToolMeta {
                    sinks: vec![],
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

    let gate = ConfirmationGate::new(
        Arc::new(broker.clone()),
        Arc::clone(&user_grants),
        Arc::clone(&audit),
        Arc::clone(&state),
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
    let gate_handle = gate.spawn();

    PassthroughRig {
        broker,
        target,
        _gate_handle: gate_handle,
        _audit: audit,
        _state: state,
        _user_grants: user_grants,
        _peer_rx: peer_rx,
        _tmp: tmp,
    }
}

fn canonical_taint() -> Vec<TaintEntry> {
    vec![
        TaintEntry {
            source: "provider".to_string(),
            detail: Some("openai".to_string()),
        },
        TaintEntry {
            source: "tool".to_string(),
            detail: Some("local/test:tool_plug@0.1.0".to_string()),
        },
    ]
}

async fn wait_for_outstanding(
    broker: &Broker,
    canonical: &CanonicalId,
    id: &JsonRpcId,
) -> rafaello_core::bus::OutstandingDispatch {
    let deadline = std::time::Instant::now() + Duration::from_secs(1);
    loop {
        if let Some(entry) = broker.peek_outstanding_for_test(canonical, id) {
            return entry;
        }
        if std::time::Instant::now() >= deadline {
            panic!("outstanding entry never populated for {id:?}");
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

#[tokio::test(flavor = "current_thread")]
async fn passthrough_arm_populates_dispatch_taint() {
    let rig = build_passthrough_rig();
    let taint = canonical_taint();
    let request_id = JsonRpcId::from(Ulid::new().to_string());

    rig.broker
        .publish_core_with_taint(
            "core.session.tool_request",
            serde_json::json!({
                "tool": "noop",
                "args": {"k": "v"},
                "dispatch_target": rig.target.to_string(),
            }),
            Some(request_id.clone()),
            None,
            Some(taint.clone()),
            None,
        )
        .expect("publish tool_request");

    let entry = wait_for_outstanding(&rig.broker, &rig.target, &request_id).await;
    assert_eq!(entry.tool_request_taint, taint);
}

#[tokio::test(flavor = "current_thread")]
async fn grant_match_arm_populates_dispatch_taint() {
    let rig = build_gate_rig();
    let taint = canonical_taint();

    let target = rig.target.clone();
    rig.user_grants.write().add(UserGrant {
        plugin: target.clone(),
        tool: "send_mail".to_string(),
        matcher: GrantMatcher::Any,
        added_at: Utc::now(),
        source: GrantSource::SlashCommand,
    });

    let request_id = JsonRpcId::from(Ulid::new().to_string());
    rig.broker
        .publish_core_with_taint(
            "core.session.tool_request",
            serde_json::json!({
                "tool": "send_mail",
                "args": {"to": "x@y.z"},
                "dispatch_target": target.to_string(),
            }),
            Some(request_id.clone()),
            None,
            Some(taint.clone()),
            None,
        )
        .expect("publish tool_request");

    let entry = wait_for_outstanding(&rig.broker, &target, &request_id).await;
    assert_eq!(entry.tool_request_taint, taint);
}

#[tokio::test(flavor = "current_thread")]
async fn post_confirm_allow_arm_populates_dispatch_taint() {
    let rig = build_gate_rig();
    let (confirm_id, tool_request_id) =
        seed_held(&rig, "send_mail", serde_json::json!({"to": "a@b.c"}));

    publish_confirm_reply(&rig.broker, &confirm_id, "allow");

    let target = CanonicalId::parse(MAILER_CANONICAL).expect("canonical");
    let entry = wait_for_outstanding(&rig.broker, &target, &tool_request_id).await;
    // `seed_held` injects a single `user`-source taint entry on the
    // held `tool_request`; CG4's allow path forwards
    // `held.tool_request.taint.clone().unwrap_or_default()` into
    // `publish_for_tool_dispatch`'s `tool_request_taint` argument.
    assert_eq!(
        entry.tool_request_taint,
        vec![TaintEntry {
            source: "user".to_string(),
            detail: None,
        }]
    );
}
