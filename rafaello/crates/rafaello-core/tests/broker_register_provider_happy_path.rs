//! `Broker::register_provider` returns an RAII `RegisteredProvider`
//! whose drop unregisters the provider (scope §B5, c09).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;

fn cid(s: &str) -> CanonicalId {
    CanonicalId::parse(s).expect("canonical id parses")
}

fn acl_with_provider(canonical: &CanonicalId, topic_id: &str, provider_id: &str) -> BrokerAcl {
    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: topic_id.to_string(),
            publish_topics: vec![],
            subscribe_patterns: vec![],
            auto_subscribes: vec![format!("plugin.{topic_id}.tool_request")],
            provider_id: Some(provider_id.to_string()),
        },
    );
    BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    }
}

#[test]
fn register_provider_then_drop_unregisters() {
    let canonical = cid("local/test:mockprov@0.1.0");
    let acl = acl_with_provider(&canonical, "mockprov_local_test", "mock");
    let broker = Broker::new(acl).expect("acl is well-formed");

    broker
        .try_reserve_provider_registration(&canonical)
        .expect("reservation precheck passes for fresh broker");

    let (peer, _rx) = fresh_peer();
    let guard = broker
        .register_provider(canonical.clone(), peer)
        .expect("registration succeeds");

    assert!(broker.contains_provider(&canonical));

    drop(guard);

    assert!(!broker.contains_provider(&canonical));
    broker
        .try_reserve_provider_registration(&canonical)
        .expect("slot is released by guard drop");
}
