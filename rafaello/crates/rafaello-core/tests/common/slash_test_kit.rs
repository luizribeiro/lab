#![allow(dead_code)]
//! Shared rig for c18 slash-handler integration tests.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::RwLock;
use rafaello_core::broker_acl::{AttachId, BrokerAcl, FrontendAcl, PluginAcl};
use rafaello_core::bus::{Broker, BusEvent, InternalSubscription, JsonRpcId};
use rafaello_core::lock::CanonicalId;
use rafaello_core::slash::SlashHandler;
use rafaello_core::user_grants::UserGrants;
use serde_json::Value;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use super::peer_test_kit::fresh_peer;
use super::reemit_test_kit::AuditRig;

pub const TUI_ATTACH_ID: &str = "tui";
pub const MAILCAT_CANONICAL: &str = "local:mailcat@0.0.0";
pub const MAILCAT_TOPIC_ID: &str = "mailcat_local";

pub fn cid(s: &str) -> CanonicalId {
    CanonicalId::parse(s).expect("canonical id parses")
}

pub fn mailcat_plugin_acl() -> PluginAcl {
    PluginAcl {
        topic_id: MAILCAT_TOPIC_ID.to_string(),
        publish_topics: vec![format!("plugin.{MAILCAT_TOPIC_ID}.tool_result")],
        subscribe_patterns: vec![],
        auto_subscribes: vec![format!("plugin.{MAILCAT_TOPIC_ID}.tool_request")],
        provider_id: None,
    }
}

pub struct SlashRig {
    pub broker: Broker,
    pub acl: Arc<BrokerAcl>,
    pub attach: AttachId,
    pub user_grants: Arc<RwLock<UserGrants>>,
    pub audit: AuditRig,
    pub shutdown_tx: watch::Sender<bool>,
    pub join: JoinHandle<()>,
}

#[derive(Default)]
pub struct SlashRigOpts {
    pub tool_routes: Vec<(&'static str, &'static str)>,
    pub plugins: Vec<(CanonicalId, PluginAcl)>,
    pub schemas: BTreeMap<String, Value>,
}

pub fn build_slash_rig(opts: SlashRigOpts) -> SlashRig {
    let attach = AttachId::new(TUI_ATTACH_ID).expect("attach id");
    let mut publish_topics = BTreeSet::new();
    publish_topics.insert(format!("frontend.{TUI_ATTACH_ID}.slash_command"));
    let frontend_acl = FrontendAcl {
        subscribe_patterns: BTreeSet::new(),
        auto_subscribes: BTreeSet::new(),
        publish_topics,
    };
    let mut frontends = BTreeMap::new();
    frontends.insert(attach.clone(), frontend_acl);

    let mut plugins: BTreeMap<CanonicalId, PluginAcl> = BTreeMap::new();
    for (c, a) in opts.plugins {
        plugins.insert(c, a);
    }
    let mut tool_routes = BTreeMap::new();
    for (tool, owner) in &opts.tool_routes {
        tool_routes.insert((*tool).to_string(), cid(owner));
    }

    let acl = BrokerAcl {
        plugins,
        tool_routes,
        frontends,
    };
    let broker = Broker::new(acl.clone()).expect("acl well-formed");
    let (peer, _rx) = fresh_peer();
    let guard = broker
        .register_frontend(attach.clone(), peer)
        .expect("frontend registers");
    Box::leak(Box::new(guard));

    let audit = AuditRig::new(&broker);
    let user_grants = Arc::new(RwLock::new(UserGrants::new()));
    let acl_arc = Arc::new(acl);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler = SlashHandler::new(
        broker.clone(),
        acl_arc.clone(),
        user_grants.clone(),
        audit.writer.clone(),
        opts.schemas,
        shutdown_rx,
    );
    let join = handler.start();

    SlashRig {
        broker,
        acl: acl_arc,
        attach,
        user_grants,
        audit,
        shutdown_tx,
        join,
    }
}

pub fn subscribe_core_command_result(
    broker: &Broker,
) -> (mpsc::Receiver<BusEvent>, InternalSubscription) {
    broker.subscribe_internal(vec!["core.session.command_result".to_string()], 64)
}

pub fn publish_slash(broker: &Broker, attach: &AttachId, payload: Value) -> JsonRpcId {
    let id = JsonRpcId::from(ulid::Ulid::new().to_string());
    let params = serde_json::json!({
        "topic": format!("frontend.{TUI_ATTACH_ID}.slash_command"),
        "payload": payload,
        "request_id": id.clone(),
    });
    broker
        .handle_frontend_publish(attach, &params)
        .expect("slash publish accepted");
    id
}

pub async fn await_command_result(rx: &mut mpsc::Receiver<BusEvent>) -> BusEvent {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    tokio::time::timeout_at(deadline, rx.recv())
        .await
        .expect("timed out waiting for command_result")
        .expect("channel closed")
}

pub async fn shutdown(rig: SlashRig) {
    rig.shutdown_tx.send(true).expect("shutdown send");
    tokio::time::timeout(Duration::from_secs(2), rig.join)
        .await
        .expect("handler exits")
        .expect("handler did not panic");
}
