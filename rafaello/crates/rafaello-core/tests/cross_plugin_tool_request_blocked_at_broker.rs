//! A non-provider plugin attempting to publish on
//! `plugin.<other-topic-id>.tool_request` is rejected via m2's
//! `PublishOnReservedNamespace` (scope §B6 step 4, c10). The c10
//! reshape adds this explicit regression check now that the error
//! machinery is `Publisher::Provider`-aware.

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn cross_plugin_tool_request_blocked() {
    let plug_a = CanonicalId::parse("local/test:plug_a@0.1.0").expect("canonical a");
    let plug_b = CanonicalId::parse("local/test:plug_b@0.1.0").expect("canonical b");
    let a_topic_id = "plug_a_local_test";
    let b_topic_id = "plug_b_local_test";

    let mut plugins = BTreeMap::new();
    plugins.insert(
        plug_a.clone(),
        PluginAcl {
            topic_id: a_topic_id.to_string(),
            publish_topics: vec![],
            subscribe_patterns: vec![],
            auto_subscribes: vec![format!("plugin.{a_topic_id}.tool_request")],
            provider_id: None,
        },
    );
    plugins.insert(
        plug_b.clone(),
        PluginAcl {
            topic_id: b_topic_id.to_string(),
            publish_topics: vec![],
            subscribe_patterns: vec![],
            auto_subscribes: vec![format!("plugin.{b_topic_id}.tool_request")],
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
        .register_plugin(plug_a.clone(), peer)
        .expect("register a");

    let bad = format!("plugin.{b_topic_id}.tool_request");
    let params = serde_json::json!({
        "topic": bad,
        "payload": {},
        "in_reply_to": [rafaello_core::bus::JsonRpcId::from("x")],
        "request_id": rafaello_core::bus::JsonRpcId::from("req-1"),
    });
    let err = broker
        .handle_plugin_publish(&plug_a, &params)
        .expect_err("must be rejected");
    assert!(
        matches!(err, BrokerError::PublishOnReservedNamespace { ref topic, .. } if topic == &bad),
        "expected PublishOnReservedNamespace, got {err:?}"
    );
}
