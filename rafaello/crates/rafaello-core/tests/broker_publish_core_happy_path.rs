//! `Broker::publish_core("core.lifecycle.test", payload)` fans out one
//! `bus.event` carrying `publisher == Core` to a subscriber of `core.**`
//! (scope §B1, §B7, §B8, c13).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn publish_core_observer_receives_bus_event_with_core_publisher() {
    let observer = CanonicalId::parse("local/test:obs@0.1.0").expect("canonical");
    let topic_id = "obs_local_test";

    let mut plugins = BTreeMap::new();
    plugins.insert(
        observer.clone(),
        PluginAcl {
            topic_id: topic_id.to_string(),
            publish_topics: vec![format!("plugin.{topic_id}.foo")],
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

    let (peer, mut rx) = fresh_peer();
    let _guard = broker
        .register_plugin(observer, peer)
        .expect("registration succeeds");

    let payload = serde_json::json!({"hello": "core"});
    broker
        .publish_core("core.lifecycle.test", payload.clone())
        .expect("publish_core happy path");

    let notification = rx
        .try_recv()
        .expect("observer receives one bus.event notification");
    assert_eq!(notification.method, "bus.event");

    let event = &notification.params;
    assert_eq!(event["topic"], "core.lifecycle.test");
    assert_eq!(event["payload"], payload);
    assert_eq!(event["publisher"], serde_json::json!({"kind": "core"}));
    assert!(event.get("in_reply_to").is_none());
    assert!(event.get("taint").is_none());

    assert!(
        rx.try_recv().is_err(),
        "no further notifications follow the single publish_core"
    );
}
