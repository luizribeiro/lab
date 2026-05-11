//! Provider publishing `provider.mock` (two-segment) is rejected as
//! `PublishOnReservedNamespace` (scope §B6 step 4, c10). The provider
//! namespace requires ≥3 segments with `segments[1] == provider_id`.

use rafaello_core::BrokerError;

mod common;
use common::provider_test_kit::provider_broker;

#[test]
fn two_segment_topic_rejected() {
    let (broker, canonical) = provider_broker();
    let bad = "provider.mock";
    let params = serde_json::json!({"topic": bad, "payload": {}});
    let err = broker
        .handle_provider_publish(&canonical, &params)
        .expect_err("must be rejected");
    assert!(
        matches!(err, BrokerError::PublishOnReservedNamespace { ref topic, .. } if topic == bad),
        "expected PublishOnReservedNamespace, got {err:?}"
    );
}
