//! Id N was dispatched to plugin A; plugin B publishing `tool_result`
//! citing N fails closed with `StaleRequestId` because the outstanding
//! map is keyed by `(target_canonical, id)` (scope §OM2, commits c10).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId};
use rafaello_core::lock::CanonicalId;
use rafaello_core::BrokerError;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn tool_result_routed_to_other_plugin_rejected() {
    let plugin_a = CanonicalId::parse("local/test:plug-a@0.1.0").expect("canonical a");
    let plugin_b = CanonicalId::parse("local/test:plug-b@0.1.0").expect("canonical b");
    let topic_id_a = "plug_a";
    let topic_id_b = "plug_b";
    let topic_b_tool_result = format!("plugin.{topic_id_b}.tool_result");

    let mut plugins = BTreeMap::new();
    plugins.insert(
        plugin_a.clone(),
        PluginAcl {
            topic_id: topic_id_a.to_string(),
            publish_topics: vec![format!("plugin.{topic_id_a}.tool_result")],
            subscribe_patterns: vec![],
            auto_subscribes: vec![format!("plugin.{topic_id_a}.tool_request")],
            provider_id: None,
        },
    );
    plugins.insert(
        plugin_b.clone(),
        PluginAcl {
            topic_id: topic_id_b.to_string(),
            publish_topics: vec![topic_b_tool_result.clone()],
            subscribe_patterns: vec![],
            auto_subscribes: vec![format!("plugin.{topic_id_b}.tool_request")],
            provider_id: None,
        },
    );
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");
    let (peer_a, _rx_a) = fresh_peer();
    let _guard_a = broker
        .register_plugin(plugin_a.clone(), peer_a)
        .expect("registered a");
    let (peer_b, _rx_b) = fresh_peer();
    let _guard_b = broker
        .register_plugin(plugin_b.clone(), peer_b)
        .expect("registered b");

    let id = JsonRpcId::from("req-42");
    broker
        .publish_for_tool_dispatch(
            &plugin_a,
            serde_json::json!({}),
            id.clone(),
            None,
            None,
            Vec::new(),
        )
        .expect("dispatch to A ok");

    let params = serde_json::json!({
        "topic": topic_b_tool_result,
        "payload": {},
        "in_reply_to": [id.clone()],
        "request_id": JsonRpcId::from("resp-1"),
    });
    let err = broker
        .handle_plugin_publish(&plugin_b, &params)
        .expect_err("B citing A's dispatched id must be rejected");
    assert!(
        matches!(
            err,
            BrokerError::StaleRequestId { canonical: ref c, id: ref i }
                if c == &plugin_b && i == &id
        ),
        "expected StaleRequestId{{plugin_b, id}}, got {err:?}"
    );
}
