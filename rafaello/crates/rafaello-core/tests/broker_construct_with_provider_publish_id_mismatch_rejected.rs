//! `Broker::new` runs §B11 defence-in-depth provider publish-id check:
//! a `PluginAcl` carrying `provider_id = Some(id)` whose `publish_topics`
//! list a `provider.<other>.*` entry (second segment != `id`) is rejected
//! at construction with `BrokerError::InvalidTopic` citing the mismatched
//! topic. Belt-and-braces against a hand-mutated ACL bypassing the m1
//! compile path (scope §B11, c13).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;
use rafaello_core::BrokerError;

#[test]
fn provider_publish_id_mismatch_rejected_at_construction() {
    let canonical = CanonicalId::parse("local/test:mockprov@0.1.0").expect("canonical id parses");
    let bad_topic = "provider.other.foo".to_string();
    let plugin_acl = PluginAcl {
        topic_id: "mockprov_local_test".to_string(),
        publish_topics: vec![format!("provider.mock.tool_request"), bad_topic.clone()],
        subscribe_patterns: vec![],
        auto_subscribes: vec![],
        provider_id: Some("mock".to_string()),
    };

    let mut plugins = BTreeMap::new();
    plugins.insert(canonical, plugin_acl);
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };

    let err = Broker::new(acl).expect_err("cross-provider publish grant must be rejected");
    match err {
        BrokerError::InvalidTopic { topic, .. } => {
            assert_eq!(topic, bad_topic, "error must cite the mismatched topic");
        }
        other => panic!("expected InvalidTopic, got {other:?}"),
    }
}
