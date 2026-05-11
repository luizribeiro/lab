//! Plugin A publishes `tool_result` twice with the same id; the first
//! succeeds (and drains the outstanding entry), the second fails at
//! intake with `StaleRequestId` (scope §OM2, commits c10).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId};
use rafaello_core::lock::CanonicalId;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn tool_result_duplicate_publish_rejected() {
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

    let id = JsonRpcId::from("req-7");
    broker
        .publish_for_tool_dispatch(&canonical, serde_json::json!({}), id.clone(), None, None)
        .expect("dispatch ok");

    let params = serde_json::json!({
        "topic": topic,
        "payload": {},
        "in_reply_to": [id.clone()],
        "request_id": JsonRpcId::from("resp-1"),
    });
    broker
        .handle_plugin_publish(&canonical, &params)
        .expect("first tool_result accepted");
    let err = broker
        .handle_plugin_publish(&canonical, &params)
        .expect_err("duplicate tool_result must be rejected");
    assert!(
        matches!(
            err,
            BrokerError::StaleRequestId { canonical: ref c, id: ref i }
                if c == &canonical && i == &id
        ),
        "expected StaleRequestId on duplicate, got {err:?}"
    );
}
