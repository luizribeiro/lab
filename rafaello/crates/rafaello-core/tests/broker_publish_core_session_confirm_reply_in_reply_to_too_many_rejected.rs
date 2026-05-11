//! `publish_core_with_taint("core.session.confirm_reply", _,
//! request_id=Some, in_reply_to=Some(vec_len>1), …)` is rejected as
//! `InvalidInReplyTo { UnexpectedMultiple }` — pi-1 M-2 exact-one
//! cardinality parallel to the `confirm_answer` test.

use std::collections::BTreeMap;

use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::{Broker, JsonRpcId};
use rafaello_core::error::{BrokerError, InReplyToReason, Publisher};

#[test]
fn publish_core_confirm_reply_in_reply_to_too_many_rejected() {
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");
    let err = broker
        .publish_core_with_taint(
            "core.session.confirm_reply",
            serde_json::json!({}),
            Some(JsonRpcId::from("req-1")),
            Some(vec![JsonRpcId::from("a"), JsonRpcId::from("b")]),
            None,
            None,
        )
        .expect_err("multi-element in_reply_to must be rejected");
    assert!(
        matches!(
            err,
            BrokerError::InvalidInReplyTo {
                publisher: Publisher::Core,
                reason: InReplyToReason::UnexpectedMultiple,
                ..
            }
        ),
        "expected InvalidInReplyTo{{Core, UnexpectedMultiple}}, got {err:?}"
    );
}
