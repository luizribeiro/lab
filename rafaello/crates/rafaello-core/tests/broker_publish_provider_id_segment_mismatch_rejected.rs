//! Provider publishing `provider.<other>.foo` where `<other>` != its
//! own `provider_id` is rejected as `PublishOnReservedNamespace`
//! (scope §B6 step 4, c10).

use rafaello_core::BrokerError;

mod common;
use common::provider_test_kit::provider_broker;

#[test]
fn id_segment_mismatch_rejected() {
    let (broker, canonical) = provider_broker();
    let bad = "provider.other.foo";
    let params = serde_json::json!({"topic": bad, "payload": {}});
    let err = broker
        .handle_provider_publish(&canonical, &params)
        .expect_err("must be rejected");
    assert!(
        matches!(err, BrokerError::PublishOnReservedNamespace { ref topic, .. } if topic == bad),
        "expected PublishOnReservedNamespace, got {err:?}"
    );
}
