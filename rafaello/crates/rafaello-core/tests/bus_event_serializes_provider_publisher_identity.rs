//! Wire-format check for the new `PublisherIdentity::Provider` variant
//! introduced in m4 c07 (scope §B3). The provider identity must
//! serialise with `kind: "provider"` and the three payload fields
//! `canonical`, `provider_id`, `topic_id`.

use rafaello_core::bus::{BusEvent, PublisherIdentity};
use serde_json::json;

#[test]
fn provider_publisher_identity_uses_kind_provider_tag() {
    let ev = BusEvent {
        topic: "provider.id_p.event".to_string(),
        payload: json!({"k": 1}),
        publisher: PublisherIdentity::Provider {
            canonical: "local/test:prov@0.1.0".to_string(),
            provider_id: "openai".to_string(),
            topic_id: "id_p".to_string(),
        },
        in_reply_to: None,
        taint: None,
        request_id: None,
    };

    let value = serde_json::to_value(&ev).expect("serialise");
    assert_eq!(
        value,
        json!({
            "topic": "provider.id_p.event",
            "payload": {"k": 1},
            "publisher": {
                "kind": "provider",
                "canonical": "local/test:prov@0.1.0",
                "provider_id": "openai",
                "topic_id": "id_p",
            },
        })
    );
}
