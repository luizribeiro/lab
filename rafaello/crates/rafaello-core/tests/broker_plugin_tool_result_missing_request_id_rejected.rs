//! Plugin publishing `plugin.<topic-id>.tool_result` with
//! `request_id: None` is rejected as `MissingRequestId` — scope §B0
//! table-of-truth enforcement applied symmetrically inside
//! `handle_plugin_publish` (c10, pi-1 B-3).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId};
use rafaello_core::error::Publisher;
use rafaello_core::lock::CanonicalId;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn plugin_tool_result_missing_request_id_rejected() {
    let canonical = CanonicalId::parse("local/test:plug@0.1.0").expect("canonical");
    let topic_id = "plug_local_test";
    let topic = format!("plugin.{topic_id}.tool_result");
    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: topic_id.to_string(),
            publish_topics: vec![topic.clone()],
            subscribe_patterns: vec![],
            auto_subscribes: vec![format!("plugin.{topic_id}.tool_request")],
            provider_id: None,
        },
    );
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");
    let (peer, _rx) = fresh_peer();
    let _guard = broker
        .register_plugin(canonical.clone(), peer)
        .expect("registered");

    let params = serde_json::json!({
        "topic": topic,
        "payload": {},
        "in_reply_to": [JsonRpcId::from("req-1")],
    });
    let err = broker
        .handle_plugin_publish(&canonical, &params)
        .expect_err("must be rejected");
    assert!(
        matches!(
            err,
            BrokerError::MissingRequestId {
                publisher: Publisher::Plugin(_),
                ..
            }
        ),
        "expected MissingRequestId{{Plugin}}, got {err:?}"
    );
}
