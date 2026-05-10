//! Plugin publishing on `core.*` is rejected as
//! `PublishOnReservedNamespace` (scope §B3 step 3, c10).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn core_namespace_rejected() {
    let canonical = CanonicalId::parse("local/test:plug@0.1.0").expect("canonical");
    let topic_id = "plug_local_test";
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
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl is well-formed");

    let (peer, _rx) = fresh_peer();
    let _guard = broker
        .register_plugin(canonical.clone(), peer)
        .expect("registration succeeds");

    let bad = "core.session.user_message";
    let params = serde_json::json!({"topic": bad, "payload": {}});
    let err = broker
        .handle_plugin_publish(&canonical, &params)
        .expect_err("must be rejected");

    assert!(
        matches!(err, BrokerError::PublishOnReservedNamespace { ref topic, .. } if topic == bad),
        "expected PublishOnReservedNamespace, got {err:?}"
    );
}
