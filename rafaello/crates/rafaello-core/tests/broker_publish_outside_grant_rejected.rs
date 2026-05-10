//! Own-namespace grant: a plugin publishing on a `plugin.<own>.*` topic
//! that is NOT in `publisher_acl.publish_topics` (even if it appears in
//! `auto_subscribes`) is rejected as `PublishOutsideGrant`
//! (scope §B3 step 3, c11).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn publish_outside_grant_rejected() {
    let canonical = CanonicalId::parse("local/test:plug_a@0.1.0").expect("canonical");
    let topic_id = "plug_a_local_test";

    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: topic_id.to_string(),
            publish_topics: vec![format!("plugin.{topic_id}.granted")],
            subscribe_patterns: vec![format!("plugin.{topic_id}.**")],
            // auto_subscribes is NOT publish authority — including the
            // topic here must not bypass the grant check.
            auto_subscribes: vec![format!("plugin.{topic_id}.ungranted")],
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

    let bad = format!("plugin.{topic_id}.ungranted");
    let params = serde_json::json!({"topic": bad, "payload": {}});
    let err = broker
        .handle_plugin_publish(&canonical, &params)
        .expect_err("must be rejected");

    assert!(
        matches!(err, BrokerError::PublishOutsideGrant { ref topic, .. } if topic == &bad),
        "expected PublishOutsideGrant, got {err:?}"
    );
}
