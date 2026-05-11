//! `plugin.<own>.tool_result` publishes with an empty `in_reply_to`
//! array are rejected as `InvalidInReplyTo { reason: EmptyArray }`
//! (scope §B6, c11).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;
use rafaello_core::{BrokerError, InReplyToReason};

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn tool_result_in_reply_to_empty_rejected() {
    let canonical = CanonicalId::parse("local/test:plug_a@0.1.0").expect("canonical");
    let topic_id = "plug_a_local_test";
    let topic = format!("plugin.{topic_id}.tool_result");

    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: topic_id.to_string(),
            publish_topics: vec![topic.clone()],
            subscribe_patterns: vec![format!("plugin.{topic_id}.**")],
            auto_subscribes: vec![],
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

    let params = serde_json::json!({
        "topic": topic,
        "payload": {},
        "in_reply_to": [],
        "request_id": rafaello_core::bus::JsonRpcId::from("req-1"),
    });
    let err = broker
        .handle_plugin_publish(&canonical, &params)
        .expect_err("must be rejected");

    assert!(
        matches!(
            err,
            BrokerError::InvalidInReplyTo {
                reason: InReplyToReason::EmptyArray,
                topic: ref t,
                ..
            } if t == &topic
        ),
        "expected InvalidInReplyTo {{ EmptyArray }}, got {err:?}"
    );
}
