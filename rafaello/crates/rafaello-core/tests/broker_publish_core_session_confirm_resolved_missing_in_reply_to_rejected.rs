//! `publish_core_with_taint("core.session.confirm_resolved", _,
//! request_id=Some, in_reply_to=None, …)` is rejected as
//! `InvalidInReplyTo { Missing }` — pi-1 M-1 new short-circuit signal
//! topic. `in_reply_to[0]` is the resolved confirm correlation id.

use std::collections::BTreeMap;

use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::{Broker, JsonRpcId};
use rafaello_core::error::{BrokerError, InReplyToReason, Publisher};

#[test]
fn publish_core_confirm_resolved_missing_in_reply_to_rejected() {
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");
    let err = broker
        .publish_core_with_taint(
            "core.session.confirm_resolved",
            serde_json::json!({}),
            Some(JsonRpcId::from("req-1")),
            None,
            None,
            None,
        )
        .expect_err("missing in_reply_to must be rejected");
    assert!(
        matches!(
            err,
            BrokerError::InvalidInReplyTo {
                publisher: Publisher::Core,
                reason: InReplyToReason::Missing,
                ..
            }
        ),
        "expected InvalidInReplyTo{{Core, Missing}}, got {err:?}"
    );
}
