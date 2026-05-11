//! c02 / pi-3 B-2 — the interior-mutable `Mutex<Option<_>>` audit slot
//! preserves the clone-visibility invariant: calling
//! `set_audit_writer` on one `Broker` handle makes the writer
//! observable through any other handle cloned from the same inner
//! `Arc<BrokerInner>`.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;

use rafaello_core::audit::{AuditKind, AuditWriter};
use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::Broker;

#[test]
fn broker_clone_sees_audit_writer_set_on_original() {
    let tmp = tempfile::tempdir().expect("state tempdir");
    let writer = AuditWriter::open_for_install(tmp.path()).expect("audit writer opens");

    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let original = Broker::new(acl).expect("acl well-formed");
    let cloned = original.clone();

    // Both handles see `None` initially.
    assert!(original.audit_writer().is_none());
    assert!(cloned.audit_writer().is_none());

    // Set on the *first* handle.
    original.set_audit_writer(writer);

    // The clone sees the same writer (Arc points into the shared inner).
    assert!(original.audit_writer().is_some());
    assert!(
        cloned.audit_writer().is_some(),
        "clone must observe writer installed on original (pi-3 B-2 clone-visibility invariant)"
    );

    // Record through the *clone*: the SQLite row materialises.
    let _ = cloned
        .record_audit_for_test(AuditKind::GrantAdded, None, &serde_json::json!({"k": 1}))
        .expect("clone has writer")
        .expect("record succeeds");
}
