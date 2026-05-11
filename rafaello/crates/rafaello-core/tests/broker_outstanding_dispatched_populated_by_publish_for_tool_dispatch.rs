//! `Broker::publish_for_tool_dispatch` populates
//! `BrokerState::outstanding_dispatched` synchronously (scope §OM1,
//! commits c10). The `#[cfg(test)]` accessor on `BrokerState` confirms
//! the map shape after a single dispatch.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId};
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn outstanding_dispatched_populated_by_publish_for_tool_dispatch() {
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

    assert_eq!(broker.outstanding_dispatched_count(&canonical), 0);
    broker
        .publish_for_tool_dispatch(
            &canonical,
            serde_json::json!({}),
            JsonRpcId::from("req-1"),
            None,
            None,
            Vec::new(),
        )
        .expect("dispatch ok");
    assert_eq!(broker.outstanding_dispatched_count(&canonical), 1);
}
