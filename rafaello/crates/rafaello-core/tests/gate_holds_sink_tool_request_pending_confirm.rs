//! c21 — a `tool_request` whose target declares
//! `sinks = ["mail"]` and no matching grant: the gate reserves a
//! held entry, publishes `core.session.confirm_request`, audits
//! `confirm_request`, and does not dispatch
//! `plugin.<id>.tool_request`.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::{Mutex, RwLock};
use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, BusEvent, JsonRpcId, TaintEntry};
use rafaello_core::compile::{
    CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan, NetworkPlan, ToolMeta,
};
use rafaello_core::gate::{ConfirmState, ConfirmationGate};
use rafaello_core::lock::canonical_id::CanonicalId;
use rafaello_core::lock::load_policy::LoadPolicy;
use rafaello_core::renderer::{RenderPipeline, RendererRegistry};
use rafaello_core::session::{SessionController, SessionStore};
use rafaello_core::user_grants::UserGrants;
use ulid::Ulid;

mod common;
use common::peer_test_kit::fresh_peer;

#[tokio::test(flavor = "current_thread")]
async fn gate_holds_sink_tool_request_pending_confirm() {
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

    let tmp = tempfile::tempdir().expect("state tempdir");
    let store = SessionStore::open(tmp.path()).expect("session store opens");
    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let controller = SessionController::new(store, pipeline, broker.clone());
    let audit = controller.audit_writer();
    let state = Arc::new(ConfirmState::new());

    let confirm_rx = Arc::new(Mutex::new(Vec::<BusEvent>::new()));
    let (mut internal_rx, _sub) =
        broker.subscribe_internal(vec!["core.session.confirm_request".to_string()], 16);
    let confirm_rx_for_task = Arc::clone(&confirm_rx);
    let collector = tokio::spawn(async move {
        while let Some(event) = internal_rx.recv().await {
            confirm_rx_for_task.lock().push(event);
        }
    });

    let gate = ConfirmationGate::new(
        Arc::new(broker.clone()),
        Arc::new(RwLock::new(UserGrants::new())),
        Arc::clone(&audit),
        Arc::clone(&state),
        compiled,
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

    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if !confirm_rx.lock().is_empty() {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("confirm_request observed within timeout");

    let observed = confirm_rx.lock().clone();
    assert_eq!(observed.len(), 1, "exactly one confirm_request fired");
    let confirm_event = observed.into_iter().next().unwrap();
    let confirm_id = confirm_event
        .request_id
        .clone()
        .expect("confirm_request carries request_id");
    assert!(
        state.is_held(&confirm_id),
        "held entry must be Active for the gate-allocated confirm_id"
    );

    assert!(
        peer_rx.try_recv().is_err(),
        "no plugin tool_request dispatched on hold path"
    );

    let conn =
        rusqlite::Connection::open(tmp.path().join("session.sqlite")).expect("readback connection");
    let kind: String = conn
        .query_row(
            "SELECT kind FROM audit_events WHERE request_id = ?1",
            [confirm_id.to_string()],
            |row| row.get(0),
        )
        .expect("audit row present");
    assert_eq!(kind, "confirm_request");

    collector.abort();
}
