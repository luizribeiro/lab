//! c31 §OP2 item 2 — for a provider plugin, dispatching
//! `core.tools_list` through the supervisor's connection service
//! returns the catalog (cloned).

#![cfg(feature = "test-fixture")]

mod common;

use std::sync::Arc;

use fittings_core::context::{DroppedNotifications, PeerHandle, ServiceContext};
use fittings_core::message::{JsonRpcId, Request};
use rafaello_core::bus::Broker;
use rafaello_core::supervisor::{PluginSupervisor, SupervisorConfig, ToolSchemaCatalog};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use common::tool_catalog_kit::{make_acl, package_dirs, single_compiled, write_openrpc};

#[tokio::test]
async fn provider_connection_serves_core_tools_list() {
    let tmp = tempfile::tempdir().unwrap();
    let canonical = common::canonical("local/test:prov@0.1.0");
    write_openrpc(
        tmp.path(),
        r#"{
          "openrpc": "1.2.6",
          "info": { "title": "prov", "version": "0.0.0" },
          "methods": [
            { "name": "do-thing", "params": [
              { "name": "x", "required": true,
                "schema": { "type": "string" } }
            ] }
          ]
        }"#,
    );

    let acl = make_acl(&canonical, &["do-thing"], Some("mock"));
    let compiled = single_compiled(&canonical, Some("mock"));
    let pkgs = package_dirs(&canonical, tmp.path());
    let catalog = Arc::new(ToolSchemaCatalog::build(&acl, &compiled, &pkgs).expect("build"));

    let broker = Broker::new(acl).expect("Broker::new");
    let sup = PluginSupervisor::new(broker, SupervisorConfig::default(), catalog);

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

    let resp = sup
        .dispatch_for_tests(canonical, req, ctx)
        .await
        .expect("dispatch ok");
    let tools = resp
        .result
        .get("tools")
        .and_then(Value::as_array)
        .expect("tools array");
    assert_eq!(tools.len(), 1);
    assert_eq!(
        tools[0].get("name").and_then(Value::as_str),
        Some("do-thing")
    );
}
