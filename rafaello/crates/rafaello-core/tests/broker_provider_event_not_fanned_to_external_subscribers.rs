//! Inbound provider events go to internal subscribers only. An
//! external plugin granted `provider.mock.**` subscribe patterns
//! does **not** receive a `bus.event` notify — internal-intake
//! never crosses into the external fan-out path. Scope §B7 + pi-1 B-5.

#![cfg(feature = "test-fixture")]

use rafaello_core::broker_acl::PluginAcl;
use rafaello_core::bus::JsonRpcId;
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;
use common::provider_test_kit::provider_broker_with_extra;

#[test]
fn provider_event_not_fanned_to_external_subscribers() {
    let ext_canonical = CanonicalId::parse("local/test:ext@0.1.0").expect("canonical ext");
    let ext_acl = PluginAcl {
        topic_id: "ext_local_test".to_string(),
        publish_topics: vec![],
        subscribe_patterns: vec!["provider.mock.**".to_string()],
        auto_subscribes: vec![],
        provider_id: None,
    };
    let (broker, provider_canonical) =
        provider_broker_with_extra(vec![(ext_canonical.clone(), ext_acl)], vec![]);

    let (peer, mut ext_rx) = fresh_peer();
    let _guard = broker
        .register_plugin(ext_canonical.clone(), peer)
        .expect("ext plugin registers");

    let observed = JsonRpcId::from("user-1");
    broker.seed_provider_observed_user_message_for_test(&provider_canonical, observed.clone());

    let params = serde_json::json!({
        "topic": "provider.mock.assistant_message",
        "payload": {"text": "hi"},
        "in_reply_to": [observed],
        "request_id": JsonRpcId::from("req-1"),
    });
    broker
        .handle_provider_publish(&provider_canonical, &params)
        .expect("publish accepted");

    assert!(
        ext_rx.try_recv().is_err(),
        "external subscriber must not receive a provider-namespace event"
    );
}
