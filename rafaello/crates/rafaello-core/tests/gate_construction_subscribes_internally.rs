//! c20 — after `spawn`, a published `core.session.tool_request`
//! reaches the gate's handler (observed via the `events_seen`
//! test hook on `ConfirmationGate`).

use std::collections::BTreeMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use parking_lot::RwLock;
use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::{Broker, JsonRpcId, TaintEntry};
use rafaello_core::gate::{ConfirmState, ConfirmationGate};
use rafaello_core::renderer::{RenderPipeline, RendererRegistry};
use rafaello_core::session::{SessionController, SessionStore};
use rafaello_core::user_grants::UserGrants;
use ulid::Ulid;

#[tokio::test(flavor = "current_thread")]
async fn gate_construction_subscribes_internally() {
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl is well-formed");

    let tmp = tempfile::tempdir().expect("state tempdir");
    let store = SessionStore::open(tmp.path()).expect("session store opens");
    let pipeline = RenderPipeline::new(Arc::new(RendererRegistry::with_builtins()));
    let controller = SessionController::new(store, pipeline, broker.clone());
    let audit = controller.audit_writer();

    let gate = ConfirmationGate::new(
        Arc::new(broker.clone()),
        Arc::new(RwLock::new(UserGrants::new())),
        Arc::clone(&audit),
        Arc::new(ConfirmState::new()),
        BTreeMap::new(),
    );
    let events_seen = gate.events_seen_handle();
    let _handle = gate.spawn();

    let request_id = JsonRpcId::from(Ulid::new().to_string());
    broker
        .publish_core_with_taint(
            "core.session.tool_request",
            serde_json::json!({
                "tool": "noop",
                "args": {},
                "dispatch_target": "local/test:absent@0.1.0",
            }),
            Some(request_id),
            None,
            Some(vec![TaintEntry {
                source: "user".to_string(),
                detail: None,
            }]),
            None,
        )
        .expect("publish tool_request");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
    while events_seen.load(Ordering::SeqCst) == 0 {
        if std::time::Instant::now() > deadline {
            panic!("gate task never observed the tool_request");
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(events_seen.load(Ordering::SeqCst), 1);
}
