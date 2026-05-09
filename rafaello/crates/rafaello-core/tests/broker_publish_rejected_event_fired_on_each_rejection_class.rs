//! Every rejection in `handle_plugin_publish` (`UnknownNamespace`,
//! `PublishOnReservedNamespace`, `PublishOutsideGrant`, `InvalidTopic`,
//! `InvalidInReplyTo`, `InvalidPayload`) emits a single
//! `core.lifecycle.publish_rejected` event with the §B9 `code` field
//! (scope §B9, c13).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::Broker;
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;

fn cid(s: &str) -> CanonicalId {
    CanonicalId::parse(s).expect("canonical id parses")
}

#[test]
fn rejection_event_fires_with_expected_code_for_each_class() {
    let publisher = cid("local/test:plug_a@0.1.0");
    let observer = cid("local/test:plug_b@0.1.0");
    let p_topic_id = "plug_a_local_test";
    let o_topic_id = "plug_b_local_test";

    let mut plugins = BTreeMap::new();
    plugins.insert(
        publisher.clone(),
        PluginAcl {
            topic_id: p_topic_id.to_string(),
            publish_topics: vec![
                format!("plugin.{p_topic_id}.granted"),
                format!("plugin.{p_topic_id}.tool_result"),
            ],
            subscribe_patterns: vec![],
            auto_subscribes: vec![],
            provider_id: None,
        },
    );
    plugins.insert(
        observer.clone(),
        PluginAcl {
            topic_id: o_topic_id.to_string(),
            publish_topics: vec![],
            subscribe_patterns: vec!["core.lifecycle.**".to_string()],
            auto_subscribes: vec![],
            provider_id: None,
        },
    );
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl is well-formed");

    let (peer_p, _rx_p) = fresh_peer();
    let (peer_o, mut rx_o) = fresh_peer();
    let _g_p = broker
        .register_plugin(publisher.clone(), peer_p)
        .expect("publisher registration");
    let _g_o = broker
        .register_plugin(observer.clone(), peer_o)
        .expect("observer registration");

    let cases: Vec<(serde_json::Value, &'static str)> = vec![
        (
            serde_json::json!({"topic": "evil.foo", "payload": {}}),
            "unknown_namespace",
        ),
        (
            serde_json::json!({"topic": "core.foo.bar", "payload": {}}),
            "publish_on_reserved_namespace",
        ),
        (
            serde_json::json!({
                "topic": format!("plugin.{p_topic_id}.notgranted"),
                "payload": {},
            }),
            "publish_outside_grant",
        ),
        (
            serde_json::json!({"topic": "", "payload": {}}),
            "invalid_topic",
        ),
        (
            serde_json::json!({
                "topic": format!("plugin.{p_topic_id}.tool_result"),
                "payload": {},
            }),
            "invalid_in_reply_to_missing",
        ),
        (
            serde_json::json!({"wat": "no topic, no payload"}),
            "invalid_payload",
        ),
    ];

    for (params, expected_code) in &cases {
        let _ = broker.handle_plugin_publish(&publisher, params);

        let notification = rx_o.try_recv().unwrap_or_else(|e| {
            panic!("observer did not receive rejection event for code `{expected_code}`: {e:?}")
        });
        assert_eq!(notification.method, "bus.event");

        let event = &notification.params;
        assert_eq!(
            event["topic"], "core.lifecycle.publish_rejected",
            "wrong topic for code `{expected_code}`"
        );
        assert_eq!(
            event["publisher"],
            serde_json::json!({"kind": "core"}),
            "rejection event must have core publisher for code `{expected_code}`"
        );
        let payload = &event["payload"];
        assert_eq!(
            payload["code"], *expected_code,
            "wrong rejection code; full event = {event}"
        );
        assert_eq!(
            payload["canonical"],
            serde_json::Value::String(publisher.to_string()),
            "rejection event canonical mismatch for code `{expected_code}`"
        );
    }

    assert!(
        rx_o.try_recv().is_err(),
        "observer should not receive any extra events beyond the six rejections"
    );
}
