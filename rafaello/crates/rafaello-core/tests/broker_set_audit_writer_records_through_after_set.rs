//! c02 — after `Broker::set_audit_writer`, `record_audit_for_test`
//! routes through the writer; the row appears in SQLite.

#![cfg(feature = "test-fixture")]

use std::collections::BTreeMap;

use rafaello_core::audit::{AuditKind, AuditWriter};
use rafaello_core::broker_acl::BrokerAcl;
use rafaello_core::bus::Broker;

#[test]
fn broker_records_audit_row_after_writer_installed() {
    let tmp = tempfile::tempdir().expect("state tempdir");
    let writer = AuditWriter::open_for_install(tmp.path()).expect("audit writer opens");

    let acl = BrokerAcl {
        plugins: BTreeMap::new(),
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    };
    let broker = Broker::new(acl).expect("acl well-formed");
    broker.set_audit_writer(writer);

    let outcome = broker
        .record_audit_for_test(
            AuditKind::GrantAdded,
            None,
            &serde_json::json!({"hello": "world"}),
        )
        .expect("writer installed, outer Option populated")
        .expect("record persists");
    assert!(outcome > 0, "audit row seq must be positive");

    let conn = rusqlite::Connection::open(
        tmp.path()
            .join(".rafaello")
            .join("state")
            .join("session.sqlite"),
    )
    .expect("readback connection");
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM audit_events", [], |row| row.get(0))
        .expect("count rows");
    assert_eq!(count, 1);
}
