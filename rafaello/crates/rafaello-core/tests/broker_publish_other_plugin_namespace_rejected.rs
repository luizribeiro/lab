//! Cross-plugin masquerade: plugin A publishing on
//! `plugin.<B-topic-id>.tool_result` is rejected as
//! `PublishOnReservedNamespace` (scope §B3 step 3, c10).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn cross_plugin_masquerade_rejected() {
    let canonical_a = CanonicalId::parse("local/test:plug_a@0.1.0").expect("canonical a");
    let topic_id_a = "plug_a_local_test";
    let topic_id_b = "plug_b_local_test";

    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical_a.clone(),
        PluginAcl {
            topic_id: topic_id_a.to_string(),
            publish_topics: vec![format!("plugin.{topic_id_a}.foo")],
            subscribe_patterns: vec![format!("plugin.{topic_id_a}.**")],
            auto_subscribes: vec![format!("plugin.{topic_id_a}.tool_request")],
            provider_id: None,
        },
    );
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl is well-formed");

    let (peer, _rx) = fresh_peer();
    let _guard = broker
        .register_plugin(canonical_a.clone(), peer)
        .expect("registration succeeds");

    let bad = format!("plugin.{topic_id_b}.tool_result");
    let params = serde_json::json!({"topic": bad, "payload": {}});
    let err = broker
        .handle_plugin_publish(&canonical_a, &params)
        .expect_err("must be rejected");

    assert!(
        matches!(err, BrokerError::PublishOnReservedNamespace { ref topic, .. } if topic == &bad),
        "expected PublishOnReservedNamespace, got {err:?}"
    );
}
