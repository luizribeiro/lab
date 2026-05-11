//! c21 — every field of the `confirm_request` payload follows the
//! §CG3 schema, with `payload.request_id == envelope.request_id`
//! (the gate-allocated confirm correlation id, per §CT0) and
//! `details.tool_call_id == held.tool_request.request_id` (the
//! tool-call correlation id, separate id space).

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
async fn gate_confirm_request_payload_matches_scope_cg3_shape() {
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
    let (peer, _peer_rx) = fresh_peer();
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

    let tool_call_id = JsonRpcId::from(Ulid::new().to_string());
    let inbound_taint = vec![TaintEntry {
        source: "user".to_string(),
        detail: Some("typed".to_string()),
    }];
    broker
        .publish_core_with_taint(
            "core.session.tool_request",
            serde_json::json!({
                "tool": "send_mail",
                "args": {"to": "a@b.c", "subject": "hi"},
                "dispatch_target": target.to_string(),
            }),
            Some(tool_call_id.clone()),
            None,
            Some(inbound_taint.clone()),
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

    let confirm_event = confirm_rx.lock().clone().into_iter().next().unwrap();
    let envelope_request_id = confirm_event
        .request_id
        .clone()
        .expect("confirm_request carries request_id");
    let in_reply_to = confirm_event
        .in_reply_to
        .clone()
        .expect("confirm_request carries in_reply_to");
    assert_eq!(in_reply_to, vec![tool_call_id.clone()]);
    let envelope_taint = confirm_event
        .taint
        .clone()
        .expect("confirm_request carries taint");
    assert_eq!(
        envelope_taint,
        vec![TaintEntry {
            source: "system".to_string(),
            detail: Some("confirm_request".to_string()),
        }]
    );

    let payload = &confirm_event.payload;
    assert_eq!(
        payload["request_id"],
        serde_json::json!(envelope_request_id.to_string()),
        "payload.request_id == envelope.request_id (gate-allocated confirm correlation id)"
    );
    assert_eq!(payload["what"], serde_json::json!("tool_call"));
    assert_eq!(
        payload["summary"],
        serde_json::json!(format!(
            "send_mail via {plugin} — sinks: [mail]",
            plugin = target
        ))
    );
    assert_eq!(payload["default"], serde_json::json!("deny"));
    assert_eq!(payload["ttl_seconds"], serde_json::json!(60));

    let details = &payload["details"];
    assert_eq!(
        details["tool_call_id"],
        serde_json::json!(tool_call_id.to_string()),
        "details.tool_call_id == held.tool_request.request_id"
    );
    assert_eq!(details["tool"], serde_json::json!("send_mail"));
    assert_eq!(
        details["args"],
        serde_json::json!({"to": "a@b.c", "subject": "hi"})
    );
    assert_eq!(details["sinks"], serde_json::json!(["mail"]));
    assert_eq!(details["always_confirm"], serde_json::json!(false));
    assert_eq!(
        details["taint"],
        serde_json::to_value(&inbound_taint).unwrap(),
        "details.taint forwarded verbatim from inbound envelope"
    );

    collector.abort();
}
