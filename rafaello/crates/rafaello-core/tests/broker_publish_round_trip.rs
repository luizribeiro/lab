//! End-to-end fan-out: A publishes `plugin.<A>.greet`; observer B
//! subscribed to `plugin.**` receives one `bus.event` matching topic,
//! payload, and publisher identity (scope §B7, §B8, c12).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn publish_round_trip_observer_receives_bus_event() {
    let a = CanonicalId::parse("local/test:plug_a@0.1.0").expect("canonical a");
    let b = CanonicalId::parse("local/test:plug_b@0.1.0").expect("canonical b");
    let a_topic_id = "plug_a_local_test";
    let b_topic_id = "plug_b_local_test";
    let topic = format!("plugin.{a_topic_id}.greet");

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
            subscribe_patterns: vec!["plugin.**".to_string()],
            auto_subscribes: vec![],
            provider_id: None,
        },
    );
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl is well-formed");

    let (peer_a, mut rx_a) = fresh_peer();
    let (peer_b, mut rx_b) = fresh_peer();
    let _guard_a = broker
        .register_plugin(a.clone(), peer_a)
        .expect("registration a succeeds");
    let _guard_b = broker
        .register_plugin(b.clone(), peer_b)
        .expect("registration b succeeds");

    let payload = serde_json::json!({"hello": "world"});
    let params = serde_json::json!({"topic": topic, "payload": payload});
    broker
        .handle_plugin_publish(&a, &params)
        .expect("publish succeeds");

    let notification = rx_b
        .try_recv()
        .expect("observer B receives one bus.event notification");
    assert_eq!(notification.method, "bus.event");

    let event = &notification.params;
    assert_eq!(event["topic"], serde_json::Value::String(topic));
    assert_eq!(event["payload"], payload);
    assert_eq!(
        event["publisher"],
        serde_json::json!({
            "kind": "plugin",
            "canonical": a.to_string(),
            "topic_id": a_topic_id,
        })
    );

    assert!(
        rx_b.try_recv().is_err(),
        "no further notifications follow the single publish"
    );
    assert!(
        rx_a.try_recv().is_err(),
        "publisher A does not receive its own publish"
    );
}
