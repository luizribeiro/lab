//! `provider.mock.assistant_message` accepts an `in_reply_to` id that
//! lives in `provider_observed_user_messages` — security RFC §7.2.6
//! row 3 (union of results + user_messages). The inbound event is
//! observed via the drain-Vec seam (scope §B6 step 9, c10).

#![cfg(feature = "test-fixture")]

use rafaello_core::bus::{JsonRpcId, PublisherIdentity};

mod common;
use common::provider_test_kit::{provider_broker, MOCK_PROVIDER_ID, MOCK_TOPIC_ID};

#[test]
fn assistant_message_user_message_id_accepted() {
    let (broker, canonical) = provider_broker();
    let user_msg_id = JsonRpcId::from("user-msg-1");
    broker.seed_provider_observed_user_message_for_test(&canonical, user_msg_id.clone());

    let topic = "provider.mock.assistant_message";
    let params = serde_json::json!({
        "topic": topic,
        "payload": {"text": "ok"},
        "in_reply_to": [user_msg_id.clone()],
        "request_id": JsonRpcId::from("req-1"),
    });
    broker
        .handle_provider_publish(&canonical, &params)
        .expect("publish accepted");

    let events = broker.drain_inbound_provider_events_for_test();
    assert_eq!(events.len(), 1, "exactly one inbound event drained");
    let event = &events[0];
    assert_eq!(event.topic, topic);
    assert_eq!(
        event.in_reply_to.as_deref(),
        Some([user_msg_id.clone()].as_slice())
    );
    assert!(event.taint.is_none(), "taint must be discarded to None");
    match &event.publisher {
        PublisherIdentity::Provider {
            canonical: c,
            provider_id,
            topic_id,
        } => {
            assert_eq!(c.as_str(), canonical.to_string().as_str());
            assert_eq!(provider_id, MOCK_PROVIDER_ID);
            assert_eq!(topic_id, MOCK_TOPIC_ID);
        }
        other => panic!("expected Provider publisher identity, got {other:?}"),
    }
}
