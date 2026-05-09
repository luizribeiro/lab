//! Registering the same canonical twice while the first guard
//! is still alive returns `AlreadyRegistered` (scope §B1, c07).

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
fn second_register_for_live_canonical_returns_already_registered() {
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
    };
    let broker = Broker::new(acl).expect("acl is well-formed");

    let (peer_a, _rx_a) = fresh_peer();
    let _guard = broker
        .register_plugin(canonical.clone(), peer_a)
        .expect("first registration succeeds");

    let (peer_b, _rx_b) = fresh_peer();
    let err = broker
        .register_plugin(canonical.clone(), peer_b)
        .expect_err("second registration for live canonical is rejected");
    assert!(matches!(err, BrokerError::AlreadyRegistered(c) if c == canonical));
}
