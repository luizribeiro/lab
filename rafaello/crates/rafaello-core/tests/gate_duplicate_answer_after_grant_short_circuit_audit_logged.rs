//! c24 / pi-2 B-4: short-circuit calls `try_resolve`, which
//! transitions `Active → ResolvedByAnswer`; `prior_outcome` then
//! classifies a subsequent answer as `Duplicate`, NOT `Late` (the
//! `TimedOut` arm — see c13's `PriorOutcome` classifier). After
//! the short-circuit fires, a stale `frontend.tui.confirm_answer`
//! for A arrives; re-emit reads `prior_outcome == Duplicate` and
//! audits `confirm_duplicate` via the c14 pipeline.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use rafaello_core::audit::AuditWriter;
use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl, PluginAcl};
use rafaello_core::bus::{Broker, BusEvent, JsonRpcId, PublisherIdentity, TaintEntry};
use rafaello_core::compile::{
    CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan, ToolMeta,
};
use rafaello_core::gate::{ConfirmState, ConfirmationGate, HeldConfirmation, PriorOutcome};
use rafaello_core::lock::canonical_id::CanonicalId;
use rafaello_core::lock::load_policy::LoadPolicy;
use rafaello_core::reemit::ReemitRouter;
use rafaello_core::renderer::{RenderPipeline, RendererRegistry};
use rafaello_core::session::{SessionController, SessionStore};
use rafaello_core::user_grants::UserGrants;
use tempfile::TempDir;
use tokio::sync::watch;
use ulid::Ulid;

mod common;
use common::peer_test_kit::fresh_peer;

const MAILER: &str = "local/test:mailer@0.1.0";
const MAILER_TOPIC: &str = "mailer_local_test";
const PROVIDER: &str = "local/test:mockprov@0.1.0";
const PROVIDER_ID: &str = "mock";
const PROVIDER_TOPIC: &str = "mockprov_local_test";
const TUI_ATTACH: &str = "tui";

struct CombinedRig {
    broker: Broker,
    state: Arc<ConfirmState>,
    user_grants: Arc<RwLock<UserGrants>>,
    audit: Arc<AuditWriter>,
    target: CanonicalId,
    attach: AttachId,
    sqlite_path: std::path::PathBuf,
    _tmp: TempDir,
}

fn build_rig() -> CombinedRig {
    let target = CanonicalId::parse(MAILER).expect("canonical");
    let provider = CanonicalId::parse(PROVIDER).expect("canonical");
    let dispatch_topic = format!("plugin.{MAILER_TOPIC}.tool_request");

    let mut plugins = BTreeMap::new();
    plugins.insert(
        target.clone(),
        PluginAcl {
            topic_id: MAILER_TOPIC.to_string(),
            publish_topics: vec![format!("plugin.{MAILER_TOPIC}.tool_result")],
            subscribe_patterns: vec![],
            auto_subscribes: vec![dispatch_topic.clone()],
            provider_id: None,
        },
    );
    plugins.insert(
        provider.clone(),
        PluginAcl {
            topic_id: PROVIDER_TOPIC.to_string(),
            publish_topics: vec![
                format!("provider.{PROVIDER_ID}.tool_request"),
                format!("provider.{PROVIDER_ID}.assistant_message"),
            ],
            subscribe_patterns: vec![],
            auto_subscribes: vec![],
            provider_id: Some(PROVIDER_ID.to_string()),
        },
    );

    let attach = AttachId::new(TUI_ATTACH).expect("attach id");
    let mut publish_topics = BTreeSet::new();
    publish_topics.insert(format!("frontend.{TUI_ATTACH}.confirm_answer"));
    let mut frontends = BTreeMap::new();
    frontends.insert(
        attach.clone(),
        FrontendAcl {
            subscribe_patterns: BTreeSet::new(),
            auto_subscribes: BTreeSet::new(),
            publish_topics,
        },
    );

    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends,
    };
    let broker = Broker::new(acl.clone()).expect("acl well-formed");

    let (peer, _rx) = fresh_peer();
    std::mem::forget(
        broker
            .register_plugin(target.clone(), peer)
            .expect("mailer registers"),
    );
    let (peer, _rx) = fresh_peer();
    std::mem::forget(
        broker
            .register_provider(provider.clone(), peer)
            .expect("provider registers"),
    );
    let (peer, _rx) = fresh_peer();
    std::mem::forget(
        broker
            .register_frontend(attach.clone(), peer)
            .expect("tui registers"),
    );

    let mut compiled = BTreeMap::new();
    compiled.insert(
        target.clone(),
        CompiledPlugin {
            canonical: target.clone(),
            topic_id: MAILER_TOPIC.to_string(),
            entry_absolute: std::path::PathBuf::from("/dev/null"),
            filesystem: FilesystemPlan::default(),
            network: NetworkPlan::Deny,
            env: EnvPlan::default(),
            limits: LimitsPlan::default(),
            subscribe_patterns: vec![],
            publish_topics: vec![],
            auto_subscribes: vec![dispatch_topic],
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
    let sqlite_path = tmp.path().join("session.sqlite");
    let store = SessionStore::open(tmp.path()).expect("session store opens");
    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let controller = SessionController::new(store, pipeline, broker.clone());
    let audit = controller.audit_writer();
    std::mem::forget(controller);
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
    std::mem::forget(gate.spawn());

    let (_shutdown_tx, shutdown_rx) = watch::channel(false);
    std::mem::forget(_shutdown_tx);
    let router = ReemitRouter::new(broker.clone(), acl, provider, shutdown_rx)
        .with_confirm_state_and_audit(Arc::clone(&state), Arc::clone(&audit));
    std::mem::forget(router.start());

    CombinedRig {
        broker,
        state,
        user_grants,
        audit,
        target,
        attach,
        sqlite_path,
        _tmp: tmp,
    }
}

fn seed_held(rig: &CombinedRig, args: serde_json::Value) -> JsonRpcId {
    let confirm_id = JsonRpcId::from(Ulid::new().to_string());
    let tool_request_id = JsonRpcId::from(Ulid::new().to_string());
    let tool_request = BusEvent {
        topic: "core.session.tool_request".into(),
        payload: serde_json::json!({
            "tool": "send_mail",
            "args": args,
            "dispatch_target": rig.target.to_string(),
        }),
        publisher: PublisherIdentity::Core,
        in_reply_to: None,
        taint: Some(vec![TaintEntry {
            source: "user".to_string(),
            detail: None,
        }]),
        request_id: Some(tool_request_id),
    };
    rig.state.reserve(
        confirm_id.clone(),
        HeldConfirmation {
            tool_request,
            deadline: Instant::now() + Duration::from_secs(60),
            dispatch_target: rig.target.clone(),
        },
    );
    confirm_id
}

fn audit_kinds_for(sqlite_path: &std::path::Path, request_id: &JsonRpcId) -> Vec<String> {
    let conn = rusqlite::Connection::open(sqlite_path).expect("audit readback");
    let mut stmt = conn
        .prepare("SELECT kind FROM audit_events WHERE request_id = ?1 ORDER BY seq")
        .expect("prepare");
    let rows = stmt
        .query_map([request_id.to_string()], |row| row.get::<_, String>(0))
        .expect("query");
    rows.filter_map(Result::ok).collect()
}

#[tokio::test(flavor = "current_thread")]
async fn gate_duplicate_answer_after_grant_short_circuit_audit_logged() {
    let rig = build_rig();
    let _ = rig.user_grants; // owned to keep grants alive
    let _ = &rig.audit;

    let confirm_a = seed_held(&rig, serde_json::json!({"to": "a@b.c"}));
    let confirm_b = seed_held(&rig, serde_json::json!({"to": "a@b.c"}));
    rig.state
        .mark_session_grant_requested(&confirm_b)
        .expect("B is Active");

    rig.broker
        .publish_core_with_taint(
            "core.session.confirm_reply",
            serde_json::json!({
                "request_id": confirm_b.to_string(),
                "answer": "allow",
            }),
            Some(JsonRpcId::from(Ulid::new().to_string())),
            Some(vec![confirm_b.clone()]),
            Some(vec![TaintEntry {
                source: "user".to_string(),
                detail: None,
            }]),
            None,
        )
        .expect("publish confirm_reply for B");

    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        if rig.state.prior_outcome(&confirm_a) == PriorOutcome::Duplicate {
            break;
        }
        if Instant::now() >= deadline {
            panic!(
                "expected A to reach Duplicate (Active → ResolvedByAnswer); got {:?}",
                rig.state.prior_outcome(&confirm_a),
            );
        }
        tokio::task::yield_now().await;
    }

    let envelope_id = JsonRpcId::from(Ulid::new().to_string());
    let params = serde_json::json!({
        "topic": format!("frontend.{TUI_ATTACH}.confirm_answer"),
        "payload": {"request_id": confirm_a.to_string(), "answer": "allow"},
        "in_reply_to": [confirm_a.clone()],
        "request_id": envelope_id,
    });
    rig.broker
        .handle_frontend_publish(&rig.attach, &params)
        .expect("frontend confirm_answer publish accepted");

    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let kinds = audit_kinds_for(&rig.sqlite_path, &confirm_a);
        if kinds.contains(&"confirm_duplicate".to_string()) {
            assert!(
                !kinds.contains(&"confirm_late".to_string()),
                "must classify as Duplicate, not Late; got {kinds:?}",
            );
            return;
        }
        if Instant::now() >= deadline {
            panic!("expected `confirm_duplicate` audit row for A; got {kinds:?}");
        }
        tokio::task::yield_now().await;
    }
}
