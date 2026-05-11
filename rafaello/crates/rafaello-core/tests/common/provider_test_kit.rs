#![allow(dead_code)]
//! Shared test fixtures for `handle_provider_publish` (c10).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, FrontendAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;

use super::peer_test_kit::fresh_peer;

pub const MOCK_PROVIDER_ID: &str = "mock";
pub const MOCK_TOPIC_ID: &str = "mockprov_local_test";
pub const MOCK_CANONICAL: &str = "local/test:mockprov@0.1.0";

pub fn cid(s: &str) -> CanonicalId {
    CanonicalId::parse(s).expect("canonical id parses")
}

pub fn mock_provider_acl() -> PluginAcl {
    PluginAcl {
        topic_id: MOCK_TOPIC_ID.to_string(),
        publish_topics: vec![
            format!("provider.{MOCK_PROVIDER_ID}.tool_request"),
            format!("provider.{MOCK_PROVIDER_ID}.assistant_message"),
        ],
        subscribe_patterns: vec![],
        auto_subscribes: vec![],
        provider_id: Some(MOCK_PROVIDER_ID.to_string()),
    }
}

pub fn provider_broker() -> (Broker, CanonicalId) {
    provider_broker_with(mock_provider_acl())
}

pub fn provider_broker_with(acl: PluginAcl) -> (Broker, CanonicalId) {
    let canonical = cid(MOCK_CANONICAL);
    let mut plugins = BTreeMap::new();
    plugins.insert(canonical.clone(), acl);
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");
    let (peer, _rx) = fresh_peer();
    let guard = broker
        .register_provider(canonical.clone(), peer)
        .expect("provider registers");
    Box::leak(Box::new(guard));
    (broker, canonical)
}

pub fn provider_broker_with_extra(
    extra_plugins: Vec<(CanonicalId, PluginAcl)>,
    extra_frontends: Vec<(rafaello_core::broker_acl::AttachId, FrontendAcl)>,
) -> (Broker, CanonicalId) {
    let canonical = cid(MOCK_CANONICAL);
    let mut plugins: BTreeMap<CanonicalId, PluginAcl> = BTreeMap::new();
    plugins.insert(canonical.clone(), mock_provider_acl());
    for (c, a) in extra_plugins {
        plugins.insert(c, a);
    }
    let frontends: BTreeMap<_, _> = extra_frontends.into_iter().collect();
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends,
    };
    let broker = Broker::new(acl).expect("acl well-formed");
    let (peer, _rx) = fresh_peer();
    let guard = broker
        .register_provider(canonical.clone(), peer)
        .expect("provider registers");
    Box::leak(Box::new(guard));
    (broker, canonical)
}
