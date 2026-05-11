//! Positive broker-level wire-shape test for
//! `core.session.confirm_resolved` (pi-2 M-1 + pi-3 M-1). A direct
//! synthetic publish is accepted; an internal subscriber observes
//! that the envelope `request_id` equals the freshly-allocated
//! resolution id, that the payload `request_id` equals
//! `in_reply_to[0]` (the resolved confirm correlation id), and that
//! no broker-side rejection fires.
//!
//! The gate-publisher positive (does the short-circuit path actually
//! publish this event with the right `reason`?) lives in c24.

use std::collections::BTreeMap;

use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::{Broker, JsonRpcId, CORE_SESSION_CONFIRM_RESOLVED};

#[test]
fn publish_core_confirm_resolved_wire_shape_positive() {
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");

    let (mut rx, _isub) =
        broker.subscribe_internal(vec![CORE_SESSION_CONFIRM_RESOLVED.to_string()], 8);

    let resolution_id = JsonRpcId::from("01JX3RES0LUT10NULID00000000");
    let confirm_id = JsonRpcId::from("01JX3C0NF1RM1DULID00000000");

    broker
        .publish_core_with_taint(
            CORE_SESSION_CONFIRM_RESOLVED,
            serde_json::json!({"request_id": confirm_id, "reason": "always_allow_session"}),
            Some(resolution_id.clone()),
            Some(vec![confirm_id.clone()]),
            None,
            None,
        )
        .expect("wire-shape-positive publish accepted");

    let event = rx
        .try_recv()
        .expect("internal subscriber observes accepted publish");
    assert_eq!(event.topic, CORE_SESSION_CONFIRM_RESOLVED);
    assert_eq!(
        event.request_id.as_ref(),
        Some(&resolution_id),
        "envelope request_id is the resolution event id",
    );
    let in_reply_to = event
        .in_reply_to
        .as_ref()
        .expect("in_reply_to present on accepted publish");
    assert_eq!(in_reply_to.len(), 1, "exactly one in_reply_to entry");
    assert_eq!(in_reply_to[0], confirm_id);
    let payload_request_id = event
        .payload
        .get("request_id")
        .and_then(serde_json::Value::as_str)
        .map(JsonRpcId::from)
        .expect("payload carries request_id");
    assert_eq!(
        payload_request_id, in_reply_to[0],
        "payload.request_id == in_reply_to[0] (the resolved confirm id)",
    );
}
