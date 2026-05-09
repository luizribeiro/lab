//! `handle_plugin_publish` must observe live registration: a plugin
//! that has been dropped from the registry receives `NotRegistered`
//! even though it remains in the broker ACL (scope §B5, c09).

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
    }
}

#[test]
fn handle_publish_after_unregister_returns_not_registered() {
    let canonical = cid("local/test:plug@0.1.0");
    let acl = acl_with(&canonical, "plug_local_test");
    let broker = Broker::new(acl).expect("acl is well-formed");

    let (peer, _rx) = fresh_peer();
    let guard = broker
        .register_plugin(canonical.clone(), peer)
        .expect("registration succeeds");
    drop(guard);

    let params = serde_json::json!({
        "topic": "plugin.plug_local_test.foo",
        "payload": {"k": "v"},
    });

    let err = broker
        .handle_plugin_publish(&canonical, &params)
        .expect_err("publish after drop must fail");

    assert!(
        matches!(err, BrokerError::NotRegistered(ref c) if c == &canonical),
        "expected NotRegistered({canonical:?}), got {err:?}"
    );
}
