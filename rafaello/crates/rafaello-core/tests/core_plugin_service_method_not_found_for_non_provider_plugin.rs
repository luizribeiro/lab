//! c31 §OP2 item 5 — a non-provider plugin's connection service
//! does NOT compose `CorePluginService`. Calling `core.tools_list`
//! through `dispatch_for_tests` returns `MethodNotFound`.

#![cfg(feature = "test-fixture")]

mod common;

use fittings_core::context::{DroppedNotifications, PeerHandle, ServiceContext};
use fittings_core::message::{JsonRpcId, Request};
use rafaello_core::bus::Broker;
use rafaello_core::supervisor::{PluginSupervisor, SupervisorConfig, ToolSchemaCatalog};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use common::tool_catalog_kit::make_acl;

#[tokio::test]
async fn non_provider_connection_returns_method_not_found_for_core_tools_list() {
    let canonical = common::canonical("local/test:tool@0.1.0");
    let acl = make_acl(&canonical, &["read-file"], None);
    let broker = Broker::new(acl).expect("Broker::new");
    let sup = PluginSupervisor::new(
        broker,
        SupervisorConfig::default(),
        ToolSchemaCatalog::empty_for_tests(),
    );

    let (tx, _rx) = mpsc::channel(8);
    let peer = PeerHandle::new(tx, DroppedNotifications::new(), CancellationToken::new());
    let ctx = ServiceContext::new(
        Some(JsonRpcId::String("req-1".into())),
        CancellationToken::new(),
        peer,
    );
    let req = Request {
        method: "core.tools_list".into(),
        params: Value::Null,
        id: Some(JsonRpcId::String("req-1".into())),
        metadata: Default::default(),
    };

    let err = sup
        .dispatch_for_tests(canonical, req, ctx)
        .await
        .expect_err("expected MethodNotFound");
    let msg = format!("{err:?}");
    assert!(
        msg.to_lowercase().contains("method") && msg.to_lowercase().contains("not"),
        "expected MethodNotFound, got {msg}"
    );
}
