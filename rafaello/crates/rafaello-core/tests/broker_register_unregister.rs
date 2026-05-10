//! `Broker::register_plugin` returns an RAII `RegisteredPlugin`
//! whose drop unregisters the plugin (scope §B1, c07).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;

fn cid(s: &str) -> CanonicalId {
    CanonicalId::parse(s).expect("canonical id parses")
}

fn acl_with(canonical: &CanonicalId, topic_id: &str) -> BrokerAcl {
    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: topic_id.to_string(),
            publish_topics: vec![format!("plugin.{topic_id}.foo")],
            subscribe_patterns: vec![format!("plugin.{topic_id}.**")],
            auto_subscribes: vec![format!("plugin.{topic_id}.tool_request")],
            provider_id: None,
        },
    );
    BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    }
}

#[test]
fn register_then_drop_unregisters_and_allows_reregistration() {
    let canonical = cid("local/test:plug@0.1.0");
    let acl = acl_with(&canonical, "plug_local_test");
    let broker = Broker::new(acl).expect("acl is well-formed");

    assert!(broker.contains_plugin(&canonical));
    broker
        .try_reserve_registration(&canonical)
        .expect("reservation precheck passes for fresh broker");

    let (peer, _rx) = fresh_peer();
    let guard = broker
        .register_plugin(canonical.clone(), peer)
        .expect("first registration succeeds");

    // While registered, try_reserve must observe the slot is taken.
    assert!(matches!(
        broker.try_reserve_registration(&canonical),
        Err(rafaello_core::BrokerError::AlreadyRegistered(_))
    ));

    drop(guard);

    // After drop, the slot is free again.
    broker
        .try_reserve_registration(&canonical)
        .expect("registration slot is released by guard drop");
    let (peer2, _rx2) = fresh_peer();
    let _guard2 = broker
        .register_plugin(canonical, peer2)
        .expect("second registration succeeds after first guard drop");
}

#[test]
fn shutdown_drains_all_registrations_and_is_idempotent() {
    let canonical = cid("local/test:plug@0.1.0");
    let acl = acl_with(&canonical, "plug_local_test");
    let broker = Broker::new(acl).expect("acl is well-formed");

    let (peer, _rx) = fresh_peer();
    let _guard = broker
        .register_plugin(canonical.clone(), peer)
        .expect("registration succeeds");

    broker.shutdown();
    broker.shutdown();

    broker
        .try_reserve_registration(&canonical)
        .expect("shutdown drained the registry");
}

#[test]
fn plugin_acl_returns_clone_for_known_canonical() {
    let canonical = cid("local/test:plug@0.1.0");
    let acl = acl_with(&canonical, "plug_local_test");
    let broker = Broker::new(acl).expect("acl is well-formed");

    let entry = broker
        .plugin_acl(&canonical)
        .expect("plugin present in ACL");
    assert_eq!(entry.topic_id, "plug_local_test");

    let other = cid("local/other:plug@0.1.0");
    assert!(broker.plugin_acl(&other).is_none());
    assert!(!broker.contains_plugin(&other));
}
