//! `Broker::publish_boot` emits `core.lifecycle.boot` to every
//! registered plugin (scope §B1, §B7, §B8, §B9; commit c08).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;

fn cid(s: &str) -> CanonicalId {
    CanonicalId::parse(s).expect("canonical id parses")
}

fn single_plugin_acl(canonical: &CanonicalId, topic_id: &str) -> BrokerAcl {
    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: topic_id.to_string(),
            publish_topics: vec![format!("plugin.{topic_id}.foo")],
            subscribe_patterns: vec!["core.lifecycle.**".to_string()],
            auto_subscribes: vec![],
            provider_id: None,
        },
    );
    BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
    }
}

#[test]
fn publish_boot_fans_out_core_lifecycle_boot_to_registered_observer() {
    let canonical = cid("local/test:obs@0.1.0");
    let acl = single_plugin_acl(&canonical, "obs_local_test");
    let broker = Broker::new(acl).expect("acl is well-formed");

    let (peer, mut rx) = fresh_peer();
    let _guard = broker
        .register_plugin(canonical, peer)
        .expect("registration succeeds");

    broker
        .publish_boot()
        .expect("publish_boot is infallible on the happy path");

    let notification = rx
        .try_recv()
        .expect("observer receives one bus.event notification");
    assert_eq!(notification.method, "bus.event");

    let event = &notification.params;
    assert_eq!(event["topic"], "core.lifecycle.boot");
    assert_eq!(event["publisher"], serde_json::json!({ "kind": "core" }));
    assert!(event.get("in_reply_to").is_none());
    assert!(event.get("taint").is_none());

    let payload = &event["payload"];
    assert_eq!(payload["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(payload["plugin_count"], 1);

    assert!(
        rx.try_recv().is_err(),
        "no further notifications follow the single boot event"
    );
}
