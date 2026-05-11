//! `Broker::publish_core_with_taint("core.session.tool_request", …,
//! taint=[{source: "provider", detail: "mock"}],
//! origin_provider=Some(<provider_canonical>))` succeeds; a subscribing
//! plugin observes the canonical event with the supplied taint while
//! the originating provider is excluded from the recipient set
//! (scope §B8 + §B10, pi-3 H-2).

use std::collections::BTreeMap;

use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, TaintEntry};
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;

#[test]
fn publish_core_with_taint_happy_path() {
    let provider = CanonicalId::parse("local/test:mockprov@0.1.0").expect("canonical");
    let provider_acl = PluginAcl {
        topic_id: "mockprov_local_test".to_string(),
        publish_topics: vec!["provider.mock.tool_request".to_string()],
        subscribe_patterns: vec!["core.session.tool_request".to_string()],
        auto_subscribes: vec![],
        provider_id: Some("mock".to_string()),
    };

    let observer = CanonicalId::parse("local/test:obs@0.1.0").expect("canonical");
    let observer_acl = PluginAcl {
        topic_id: "obs_local_test".to_string(),
        publish_topics: vec![],
        subscribe_patterns: vec!["core.session.**".to_string()],
        auto_subscribes: vec![],
        provider_id: None,
    };

    let mut plugins = BTreeMap::new();
    plugins.insert(provider.clone(), provider_acl);
    plugins.insert(observer.clone(), observer_acl);
    let acl = BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");

    let (provider_peer, mut provider_rx) = fresh_peer();
    let _g_prov = broker
        .register_provider(provider.clone(), provider_peer)
        .expect("provider registers");

    let (observer_peer, mut observer_rx) = fresh_peer();
    let _g_obs = broker
        .register_plugin(observer.clone(), observer_peer)
        .expect("observer registers");

    let taint = vec![TaintEntry {
        source: "provider".to_string(),
        detail: Some("mock".to_string()),
    }];
    broker
        .publish_core_with_taint(
            "core.session.tool_request",
            serde_json::json!({"tool": "echo"}),
            Some("req-1".into()),
            None,
            Some(taint.clone()),
            Some(provider.clone()),
        )
        .expect("happy path");

    let notification = observer_rx
        .try_recv()
        .expect("observer receives canonical tool_request");
    assert_eq!(notification.method, "bus.event");
    let event = &notification.params;
    assert_eq!(event["topic"], "core.session.tool_request");
    assert_eq!(
        event["taint"],
        serde_json::json!([{"source": "provider", "detail": "mock"}])
    );
    assert_eq!(event["publisher"], serde_json::json!({"kind": "core"}));

    assert!(
        provider_rx.try_recv().is_err(),
        "originating provider must be excluded from fan-out (pi-3 H-2)"
    );
}
