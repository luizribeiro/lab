//! Plugin publishing `plugin.<topic-id>.tool_result` without
//! `in_reply_to` is rejected as `InvalidInReplyTo { Missing }` —
//! regression check that m2's enforcement continues to fire under the
//! m4 reshape (scope §I; pi-1 B-3 named the gap).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId};
use rafaello_core::error::{InReplyToReason, Publisher};
use rafaello_core::lock::CanonicalId;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn plugin_tool_result_missing_in_reply_to_rejected() {
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
        "request_id": JsonRpcId::from("req-1"),
    });
    let err = broker
        .handle_plugin_publish(&canonical, &params)
        .expect_err("must be rejected");
    assert!(
        matches!(
            err,
            BrokerError::InvalidInReplyTo {
                publisher: Publisher::Plugin(_),
                reason: InReplyToReason::Missing,
                ..
            }
        ),
        "expected InvalidInReplyTo{{Plugin, Missing}}, got {err:?}"
    );
}
