//! `Broker::register_provider` rejects a second registration for
//! the same canonical with `ProviderAlreadyRegistered` (scope §B5, c09).

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
fn duplicate_registration_yields_already_registered() {
    let canonical = cid("local/test:mockprov@0.1.0");
    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: "mockprov_local_test".to_string(),
            publish_topics: vec![],
            subscribe_patterns: vec![],
            auto_subscribes: vec!["plugin.mockprov_local_test.tool_request".to_string()],
            provider_id: Some("mock".to_string()),
        },
    );
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl is well-formed");

    let (peer1, _rx1) = fresh_peer();
    let _guard = broker
        .register_provider(canonical.clone(), peer1)
        .expect("first registration succeeds");

    assert!(matches!(
        broker.try_reserve_provider_registration(&canonical),
        Err(BrokerError::ProviderAlreadyRegistered(_))
    ));

    let (peer2, _rx2) = fresh_peer();
    assert!(matches!(
        broker.register_provider(canonical, peer2),
        Err(BrokerError::ProviderAlreadyRegistered(_))
    ));
}
