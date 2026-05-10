//! Two-segment `plugin.<id>` topics are grammar-valid but
//! semantically empty; they are rejected as
//! `PublishOnReservedNamespace` regardless of whose `<id>`
//! they reference (scope §B3 step 3, c10; pi-1 §8, §306).

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
        frontends: BTreeMap::new(),
    }
}

fn assert_reserved(bad_topic: &str) {
    let canonical = cid("local/test:plug@0.1.0");
    let acl = acl_with(&canonical, "plug_local_test");
    let broker = Broker::new(acl).expect("acl is well-formed");

    let (peer, _rx) = fresh_peer();
    let _guard = broker
        .register_plugin(canonical.clone(), peer)
        .expect("registration succeeds");

    let params = serde_json::json!({
        "topic": bad_topic,
        "payload": {"k": "v"},
    });

    let err = broker
        .handle_plugin_publish(&canonical, &params)
        .expect_err("topic must be rejected");

    assert!(
        matches!(err, BrokerError::PublishOnReservedNamespace { ref topic, .. } if topic == bad_topic),
        "expected PublishOnReservedNamespace for {bad_topic:?}, got {err:?}"
    );
}

#[test]
fn own_id_two_segment_rejected() {
    assert_reserved("plugin.plug_local_test");
}

#[test]
fn other_id_two_segment_rejected() {
    assert_reserved("plugin.other_topic_id");
}
