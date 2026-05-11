//! `fan_out` of a canonical `core.session.tool_result` to a registered
//! provider populates `provider_observed_results[provider]` so the
//! provider can later cite the result id in its own `tool_request`
//! (scope §B8 + §B7b round-trip).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, JsonRpcId, TaintEntry};
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn fan_out_populates_provider_observed_results() {
    let provider = CanonicalId::parse("local/test:mockprov@0.1.0").expect("canonical");
    let provider_acl = PluginAcl {
        topic_id: "mockprov_local_test".to_string(),
        publish_topics: vec!["provider.mock.tool_request".to_string()],
        subscribe_patterns: vec!["core.session.tool_result".to_string()],
        auto_subscribes: vec![],
        provider_id: Some("mock".to_string()),
    };

    let mut plugins = BTreeMap::new();
    plugins.insert(provider.clone(), provider_acl);
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");

    let (peer, mut rx) = fresh_peer();
    let _g = broker
        .register_provider(provider.clone(), peer)
        .expect("provider registers");

    let result_id = JsonRpcId::from("tr-fan-1");
    let taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some("local/test:tooler@0.1.0".to_string()),
    }];
    broker
        .publish_core_with_taint(
            "core.session.tool_result",
            serde_json::json!({"ok": true}),
            Some(result_id.clone()),
            Some(vec![JsonRpcId::from("orig-req")]),
            Some(taint),
            None,
        )
        .expect("tool_result publish succeeds");

    let notification = rx.try_recv().expect("provider receives tool_result");
    assert_eq!(notification.method, "bus.event");
    assert_eq!(notification.params["topic"], "core.session.tool_result");

    let provider_taint = vec![TaintEntry {
        source: "provider".to_string(),
        detail: Some("mock".to_string()),
    }];
    let params = serde_json::json!({
        "topic": "provider.mock.tool_request",
        "payload": {"tool": "next"},
        "request_id": JsonRpcId::from("req-next"),
        "in_reply_to": [result_id.clone()],
        "taint": provider_taint,
    });
    broker
        .handle_provider_publish(&provider, &params)
        .expect("provider publish cites observed result id");
}
