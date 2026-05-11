//! c02 — Initial-state `Broker.audit` is `None`; `record_audit_for_test`
//! silently drops the call without touching any writer.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;

use rafaello_core::audit::AuditKind;
use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::Broker;

#[test]
fn broker_initial_audit_slot_drops_call_silently() {
    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");

    assert!(broker.audit_writer().is_none());
    let outcome =
        broker.record_audit_for_test(AuditKind::GrantAdded, None, &serde_json::json!({"a": 1}));
    assert!(
        outcome.is_none(),
        "record_audit_for_test must drop silently when no writer is installed"
    );
}
