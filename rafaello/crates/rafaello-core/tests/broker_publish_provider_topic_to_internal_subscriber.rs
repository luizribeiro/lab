//! Positive path for `handle_provider_publish`: a provider publishes
//! `provider.mock.assistant_message` referencing a seeded observed
//! result; the internal subscriber observes a `BusEvent` with
//! `publisher: Provider {..}`, `request_id: Some(_)`, `taint: None`.
//! No external plugin or frontend recipient `notify` count increments.
//! Scope §B7 + pi-1 B-2 (moved from c10).

#![cfg(feature = "test-fixture")]

use rafaello_core::broker_acl::{AttachId, FrontendAcl, PluginAcl};
use rafaello_core::bus::{JsonRpcId, PublisherIdentity};
use rafaello_core::lock::CanonicalId;

mod common;
use common::peer_test_kit::fresh_peer;
use common::provider_test_kit::{provider_broker_with_extra, MOCK_PROVIDER_ID, MOCK_TOPIC_ID};

#[test]
fn publish_provider_topic_to_internal_subscriber() {
    let other_plugin = CanonicalId::parse("local/test:other@0.1.0").expect("canonical other");
    let other_acl = PluginAcl {
        topic_id: "other_local_test".to_string(),
        publish_topics: vec![],
        subscribe_patterns: vec!["provider.**".to_string()],
        auto_subscribes: vec![],
        provider_id: None,
    };

    let other_provider = CanonicalId::parse("local/test:provb@0.1.0").expect("canonical provb");
    let other_provider_acl = PluginAcl {
        topic_id: "provb_local_test".to_string(),
        publish_topics: vec!["provider.altprov.assistant_message".to_string()],
        subscribe_patterns: vec!["provider.**".to_string()],
        auto_subscribes: vec![],
        provider_id: Some("altprov".to_string()),
    };

    let frontend_id = AttachId::new("tui").expect("attach id");
    let frontend_acl = FrontendAcl {
        publish_topics: Default::default(),
        subscribe_patterns: ["provider.**".to_string()].into_iter().collect(),
        auto_subscribes: Default::default(),
    };

    let (broker, provider_canonical) = provider_broker_with_extra(
        vec![
            (other_plugin.clone(), other_acl),
            (other_provider.clone(), other_provider_acl),
        ],
        vec![(frontend_id.clone(), frontend_acl)],
    );

    let (peer_other, mut rx_other) = fresh_peer();
    let _g_other = broker
        .register_plugin(other_plugin.clone(), peer_other)
        .expect("other plugin registers");

    let (peer_other_prov, mut rx_other_prov) = fresh_peer();
    let _g_other_prov = broker
        .register_provider(other_provider.clone(), peer_other_prov)
        .expect("other provider registers");

    let (peer_fe, mut rx_fe) = fresh_peer();
    let _g_fe = broker
        .register_frontend(frontend_id, peer_fe)
        .expect("frontend registers");

    let result_id = JsonRpcId::from("tr-1");
    broker.seed_provider_observed_result_for_test(&provider_canonical, result_id.clone());

    let (mut internal_rx, _isub) = broker.subscribe_internal(vec!["provider.**".to_string()], 8);

    let topic = "provider.mock.assistant_message";
    let params = serde_json::json!({
        "topic": topic,
        "payload": {"text": "ok"},
        "in_reply_to": [result_id.clone()],
        "request_id": JsonRpcId::from("req-1"),
    });
    broker
        .handle_provider_publish(&provider_canonical, &params)
        .expect("publish accepted");

    let event = internal_rx
        .try_recv()
        .expect("internal subscriber received event");
    assert_eq!(event.topic, topic);
    assert!(event.request_id.is_some(), "request_id forwarded");
    assert!(event.taint.is_none(), "taint discarded");
    match &event.publisher {
        PublisherIdentity::Provider {
            canonical,
            provider_id,
            topic_id,
        } => {
            assert_eq!(canonical.as_str(), provider_canonical.to_string().as_str());
            assert_eq!(provider_id, MOCK_PROVIDER_ID);
            assert_eq!(topic_id, MOCK_TOPIC_ID);
        }
        other => panic!("expected Provider publisher, got {other:?}"),
    }

    assert!(
        rx_other.try_recv().is_err(),
        "external plugin must not receive provider-namespace event"
    );
    assert!(
        rx_other_prov.try_recv().is_err(),
        "other provider must not receive provider-namespace event"
    );
    assert!(
        rx_fe.try_recv().is_err(),
        "frontend must not receive provider-namespace event"
    );
}
