//! c04 — `Broker::publish_for_tool_dispatch` records the canonical
//! `tool_request_taint` argument on the inserted
//! [`OutstandingDispatch`] entry (scope §PT1 data model).
//!
//! The `peek_outstanding_for_test` seam reads the live entry back so
//! the populator + inspector contract is checked end-to-end without
//! consuming the entry (c14 will read it during plugin `tool_result`
//! enforcement).

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId, TaintEntry};
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn outstanding_dispatch_carries_request_taint() {
    let canonical = CanonicalId::parse("local/test:plug@0.1.0").expect("canonical");
    let topic_id = "plug_local_test";
    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: topic_id.to_string(),
            publish_topics: vec![format!("plugin.{topic_id}.tool_result")],
            subscribe_patterns: vec![],
            auto_subscribes: vec![format!("plugin.{topic_id}.tool_request")],
            provider_id: None,
        },
    );
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");
    let (peer, _rx) = fresh_peer();
    let _guard = broker
        .register_plugin(canonical.clone(), peer)
        .expect("registered");

    let id = JsonRpcId::from("req-c04");
    let canonical_taint = vec![TaintEntry {
        source: "provider".to_string(),
        detail: Some("openai".to_string()),
    }];

    broker
        .publish_for_tool_dispatch(
            &canonical,
            serde_json::json!({}),
            id.clone(),
            None,
            None,
            canonical_taint.clone(),
        )
        .expect("dispatch ok");

    let entry = broker
        .peek_outstanding_for_test(&canonical, &id)
        .expect("outstanding entry present");
    assert_eq!(entry.tool_request_taint, canonical_taint);
}
