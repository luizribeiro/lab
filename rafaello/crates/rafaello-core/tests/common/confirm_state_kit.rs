//! Shared fixture for c13 `ConfirmState` integration tests.

use std::time::{Duration, Instant};

use rafaello_core::bus::{BusEvent, PublisherIdentity};
use rafaello_core::gate::HeldConfirmation;
use rafaello_core::lock::canonical_id::CanonicalId;

pub fn held() -> HeldConfirmation {
    HeldConfirmation {
        tool_request: BusEvent {
            topic: "plugin.x.tool_request".into(),
            payload: serde_json::Value::Null,
            publisher: PublisherIdentity::Core,
            in_reply_to: None,
            taint: None,
            request_id: None,
        },
        deadline: Instant::now() + Duration::from_secs(60),
        dispatch_target: CanonicalId::parse("local/test:tool@0.1.0").unwrap(),
    }
}
