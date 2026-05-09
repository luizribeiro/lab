//! Unsubscribed drop: A publishes `plugin.<A>.foo`; B subscribed only
//! to `core.**` does NOT receive it (scope §B7, c12).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn unsubscribed_plugin_does_not_receive_unrelated_publish() {
    let a = CanonicalId::parse("local/test:plug_a@0.1.0").expect("canonical a");
    let b = CanonicalId::parse("local/test:plug_b@0.1.0").expect("canonical b");
    let a_topic_id = "plug_a_local_test";
    let b_topic_id = "plug_b_local_test";
    let topic = format!("plugin.{a_topic_id}.foo");

    let mut plugins = BTreeMap::new();
    plugins.insert(
        a.clone(),
        PluginAcl {
            topic_id: a_topic_id.to_string(),
            publish_topics: vec![topic.clone()],
            subscribe_patterns: vec![],
            auto_subscribes: vec![],
            provider_id: None,
        },
    );
    plugins.insert(
        b.clone(),
        PluginAcl {
            topic_id: b_topic_id.to_string(),
            publish_topics: vec![],
            subscribe_patterns: vec!["core.**".to_string()],
            auto_subscribes: vec![],
            provider_id: None,
        },
    );
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl is well-formed");

    let (peer_a, _rx_a) = fresh_peer();
    let (peer_b, mut rx_b) = fresh_peer();
    let _guard_a = broker
        .register_plugin(a.clone(), peer_a)
        .expect("registration a succeeds");
    let _guard_b = broker
        .register_plugin(b.clone(), peer_b)
        .expect("registration b succeeds");

    let params = serde_json::json!({"topic": topic, "payload": {}});
    broker
        .handle_plugin_publish(&a, &params)
        .expect("publish succeeds");

    assert!(
        rx_b.try_recv().is_err(),
        "B (subscribed only to core.**) must not receive a plugin.* event"
    );
}
