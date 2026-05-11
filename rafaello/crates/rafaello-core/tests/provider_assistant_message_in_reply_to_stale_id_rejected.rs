//! Provider publishing `provider.mock.assistant_message` citing an
//! id absent from BOTH `provider_observed_results` and
//! `provider_observed_user_messages` is rejected as
//! `InvalidInReplyTo { StaleRequestId }` (scope §B6 step 7, c10).

use rafaello_core::bus::JsonRpcId;
use rafaello_core::error::InReplyToReason;
use rafaello_core::BrokerError;

mod common;
use common::provider_test_kit::provider_broker;

#[test]
fn assistant_message_stale_id_rejected() {
    let (broker, canonical) = provider_broker();
    let topic = "provider.mock.assistant_message";
    let unknown = JsonRpcId::from("never-seen");
    let params = serde_json::json!({
        "topic": topic,
        "payload": {"text": "hi"},
        "in_reply_to": [unknown.clone()],
        "request_id": JsonRpcId::from("req-1"),
    });
    let err = broker
        .handle_provider_publish(&canonical, &params)
        .expect_err("must be rejected");
    assert!(
        matches!(
            err,
            BrokerError::InvalidInReplyTo {
                reason: InReplyToReason::StaleRequestId { ref id },
                ..
            } if id == &unknown
        ),
        "expected InvalidInReplyTo{{StaleRequestId}}, got {err:?}"
    );
}
