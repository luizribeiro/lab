//! `Broker::register_provider` rejects unknown canonicals and
//! known canonicals whose `PluginAcl.provider_id` is `None`
//! (scope §B5, c09).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

fn cid(s: &str) -> CanonicalId {
    CanonicalId::parse(s).expect("canonical id parses")
}

#[test]
fn unknown_canonical_yields_provider_not_in_acl() {
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("empty acl is well-formed");
    let canonical = cid("local/test:ghost@0.1.0");

    assert!(matches!(
        broker.try_reserve_provider_registration(&canonical),
        Err(BrokerError::ProviderNotInAcl(_))
    ));

    let (peer, _rx) = fresh_peer();
    assert!(matches!(
        broker.register_provider(canonical, peer),
        Err(BrokerError::ProviderNotInAcl(_))
    ));
}

#[test]
fn known_canonical_without_provider_id_yields_provider_not_in_acl() {
    let canonical = cid("local/test:plug@0.1.0");
    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: "plug_local_test".to_string(),
            publish_topics: vec![],
            subscribe_patterns: vec![],
            auto_subscribes: vec!["plugin.plug_local_test.tool_request".to_string()],
            provider_id: None,
        },
    );
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl is well-formed");

    assert!(matches!(
        broker.try_reserve_provider_registration(&canonical),
        Err(BrokerError::ProviderNotInAcl(_))
    ));

    let (peer, _rx) = fresh_peer();
    assert!(matches!(
        broker.register_provider(canonical, peer),
        Err(BrokerError::ProviderNotInAcl(_))
    ));
}
