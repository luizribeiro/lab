//! `publish_core_with_taint("core.session.confirm_resolved", _,
//! request_id=None, …)` is rejected with `MissingRequestId` — pi-1 M-1
//! short-circuit signal topic added to the §B0 suffix list.

use std::collections::BTreeMap;

use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::Broker;
use rafaello_core::error::BrokerError;

#[test]
fn publish_core_confirm_resolved_missing_request_id_rejected() {
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
            None,
            None,
            None,
            None,
        )
        .expect_err("missing request_id must be rejected");
    match err {
        BrokerError::MissingRequestId { topic, .. } => {
            assert_eq!(topic, "core.session.confirm_resolved");
        }
        other => panic!("expected MissingRequestId, got {other:?}"),
    }
}
