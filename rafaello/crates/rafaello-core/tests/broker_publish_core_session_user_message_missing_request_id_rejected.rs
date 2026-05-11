//! `publish_core_with_taint("core.session.user_message", _,
//! request_id=None, …)` is rejected with `MissingRequestId` — the §B0
//! table-of-truth applies to canonical user messages even though
//! they carry no taint.

use std::collections::BTreeMap;

use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::Broker;
use rafaello_core::error::BrokerError;

#[test]
fn publish_core_user_message_missing_request_id_rejected() {
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");

    let err = broker
        .publish_core_with_taint(
            "core.session.user_message",
            serde_json::json!({"text": "hi"}),
            None,
            None,
            None,
            None,
        )
        .expect_err("missing request_id must be rejected");

    match err {
        BrokerError::MissingRequestId { topic, .. } => {
            assert_eq!(topic, "core.session.user_message");
        }
        other => panic!("expected MissingRequestId, got {other:?}"),
    }
}
