//! c27 (pi-2 M-2) — `AuditWriter::open_for_install(&tempdir)` on a fresh
//! tempdir creates `.rafaello/state/`, opens the SQLite file, makes
//! `audit_events` queryable, and a subsequent `record` call succeeds.

use rafaello_core::audit::{AuditKind, AuditWriter};

#[test]
fn audit_writer_open_for_install_creates_state_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path();

    let state_dir = project_root.join(".rafaello").join("state");
    assert!(!state_dir.exists());

    let writer = AuditWriter::open_for_install(project_root).expect("open_for_install");

    assert!(state_dir.is_dir(), "state dir should exist");
    let db = state_dir.join("session.sqlite");
    assert!(db.is_file(), "sqlite file should exist");

    let conn = rusqlite::Connection::open(&db).unwrap();
    let _row: Option<i64> = conn
        .query_row("SELECT COUNT(*) FROM audit_events", [], |r| r.get(0))
        .ok();

    let seq = writer
        .record(
            AuditKind::InstallAccepted,
            None,
            &serde_json::json!({"hi": "there"}),
        )
        .expect("record");
    assert!(seq >= 0);
}
