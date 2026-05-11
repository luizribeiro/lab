//! pi-3 H-2: when `publish_core_with_taint` is called with
//! `origin_provider = Some(provider_a)`, fan-out of the canonical
//! `core.session.tool_request` excludes provider A while still
//! reaching provider B (scope §B8 + §B10).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, TaintEntry};
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn publish_core_with_taint_excludes_origin_provider() {
    let provider_a = CanonicalId::parse("local/test:prova@0.1.0").expect("canonical");
    let provider_a_acl = PluginAcl {
        topic_id: "prova_local_test".to_string(),
        publish_topics: vec!["provider.alpha.tool_request".to_string()],
        subscribe_patterns: vec!["core.session.tool_request".to_string()],
        auto_subscribes: vec![],
        provider_id: Some("alpha".to_string()),
    };

    let provider_b = CanonicalId::parse("local/test:provb@0.1.0").expect("canonical");
    let provider_b_acl = PluginAcl {
        topic_id: "provb_local_test".to_string(),
        publish_topics: vec!["provider.beta.tool_request".to_string()],
        subscribe_patterns: vec!["core.session.tool_request".to_string()],
        auto_subscribes: vec![],
        provider_id: Some("beta".to_string()),
    };

    let mut plugins = BTreeMap::new();
    plugins.insert(provider_a.clone(), provider_a_acl);
    plugins.insert(provider_b.clone(), provider_b_acl);
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");

    let (peer_a, mut rx_a) = fresh_peer();
    let _g_a = broker
        .register_provider(provider_a.clone(), peer_a)
        .expect("provider a registers");

    let (peer_b, mut rx_b) = fresh_peer();
    let _g_b = broker
        .register_provider(provider_b.clone(), peer_b)
        .expect("provider b registers");

    let taint = vec![TaintEntry {
        source: "provider".to_string(),
        detail: Some("alpha".to_string()),
    }];
    broker
        .publish_core_with_taint(
            "core.session.tool_request",
            serde_json::json!({"tool": "echo"}),
            Some("req-1".into()),
            None,
            Some(taint),
            Some(provider_a.clone()),
        )
        .expect("publish succeeds");

    assert!(
        rx_a.try_recv().is_err(),
        "provider A (origin) excluded from fan-out"
    );
    let notification = rx_b
        .try_recv()
        .expect("provider B receives canonical tool_request");
    assert_eq!(notification.method, "bus.event");
    assert_eq!(notification.params["topic"], "core.session.tool_request");
}
