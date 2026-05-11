//! Plugin A publishes `tool_result` citing an id nothing was
//! dispatched for; broker rejects with `StaleRequestId` at intake
//! (scope §OM2, commits c10).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId};
use rafaello_core::lock::CanonicalId;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn plugin_tool_result_unknown_in_reply_to_rejected() {
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

    let unknown = JsonRpcId::from("never-dispatched");
    let params = serde_json::json!({
        "topic": topic,
        "payload": {},
        "in_reply_to": [unknown.clone()],
        "request_id": JsonRpcId::from("req-1"),
    });
    let err = broker
        .handle_plugin_publish(&canonical, &params)
        .expect_err("must be rejected");
    assert!(
        matches!(
            err,
            BrokerError::StaleRequestId { canonical: ref c, id: ref i }
                if c == &canonical && i == &unknown
        ),
        "expected StaleRequestId{{canonical, unknown}}, got {err:?}"
    );
}
