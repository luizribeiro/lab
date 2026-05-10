//! `Broker::register_plugin` rejects canonical ids that are not
//! present in the ACL with `BrokerError::NotInAcl` (scope §B1, c07).

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
fn register_unknown_canonical_returns_not_in_acl() {
    let known = cid("local/test:known@0.1.0");
    let unknown = cid("local/test:unknown@0.1.0");

    let mut plugins = BTreeMap::new();
    plugins.insert(
        known.clone(),
        PluginAcl {
            topic_id: "known_local_test".to_string(),
            publish_topics: vec![],
            subscribe_patterns: vec![],
            auto_subscribes: vec!["plugin.known_local_test.tool_request".to_string()],
            provider_id: None,
        },
    );
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl is well-formed");

    assert!(broker.contains_plugin(&known));
    assert!(!broker.contains_plugin(&unknown));

    assert!(matches!(
        broker.try_reserve_registration(&unknown),
        Err(BrokerError::NotInAcl(c)) if c == unknown
    ));

    let (peer, _rx) = fresh_peer();
    let err = broker
        .register_plugin(unknown.clone(), peer)
        .expect_err("unknown canonical is rejected");
    assert!(matches!(err, BrokerError::NotInAcl(c) if c == unknown));
}
