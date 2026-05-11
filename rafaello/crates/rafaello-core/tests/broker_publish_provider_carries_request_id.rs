//! `PublishMsg.request_id` round-trips into `BusEvent.request_id`
//! across the broker fan-out (scope §B1, c07). The provider-publish
//! handler does not exist yet (it lands in c10) — for the cutover we
//! observe the round-trip on the existing plugin-publish path.

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId};
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn plugin_publish_request_id_round_trips_into_bus_event() {
    let a = CanonicalId::parse("local/test:plug_a@0.1.0").expect("canonical a");
    let b = CanonicalId::parse("local/test:plug_b@0.1.0").expect("canonical b");
    let a_topic_id = "plug_a_local_test";
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
            topic_id: "plug_b_local_test".to_string(),
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
    let broker = Broker::new(acl).expect("acl well-formed");

    let (peer_a, _rx_a) = fresh_peer();
    let (peer_b, mut rx_b) = fresh_peer();
    let _guard_a = broker
        .register_plugin(a.clone(), peer_a)
        .expect("register a");
    let _guard_b = broker
        .register_plugin(b.clone(), peer_b)
        .expect("register b");

    let request_id = JsonRpcId::from("req-42");
    let params = serde_json::json!({
        "topic": topic,
        "payload": {"k": 1},
        "request_id": request_id,
    });
    broker
        .handle_plugin_publish(&a, &params)
        .expect("publish succeeds");

    let notification = rx_b.try_recv().expect("observer receives bus.event");
    assert_eq!(notification.method, "bus.event");
    let event = &notification.params;
    assert_eq!(event["request_id"], serde_json::json!("req-42"));
}

#[test]
fn plugin_publish_omits_request_id_when_absent() {
    let a = CanonicalId::parse("local/test:plug_a@0.1.0").expect("canonical a");
    let b = CanonicalId::parse("local/test:plug_b@0.1.0").expect("canonical b");
    let a_topic_id = "plug_a_local_test";
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
            topic_id: "plug_b_local_test".to_string(),
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
    let broker = Broker::new(acl).expect("acl well-formed");

    let (peer_a, _rx_a) = fresh_peer();
    let (peer_b, mut rx_b) = fresh_peer();
    let _guard_a = broker
        .register_plugin(a.clone(), peer_a)
        .expect("register a");
    let _guard_b = broker
        .register_plugin(b.clone(), peer_b)
        .expect("register b");

    let params = serde_json::json!({"topic": topic, "payload": null});
    broker
        .handle_plugin_publish(&a, &params)
        .expect("publish succeeds");

    let notification = rx_b.try_recv().expect("observer receives bus.event");
    let obj = notification.params.as_object().expect("event is object");
    assert!(
        !obj.contains_key("request_id"),
        "request_id must be omitted when None: {obj:?}"
    );
}
