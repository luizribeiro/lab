//! c08 — opening a fresh session store creates the `audit_events`
//! SQLite table with the four columns from scope §AL1.

use rafaello_core::session::SessionStore;

#[test]
fn audit_table_migration_creates_audit_events() {
    let tmp = tempfile::tempdir().expect("state tempdir");
    let store = SessionStore::open(tmp.path()).expect("session store opens");

    let db_path = tmp.path().join("session.sqlite");
    let conn = rusqlite::Connection::open(&db_path).expect("open db readback");

    let mut stmt = conn
        .prepare("PRAGMA table_info(audit_events)")
        .expect("table_info");
    let cols: Vec<(String, String, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .expect("query_map")
        .map(|r| r.expect("row"))
        .collect();

    let names: Vec<&str> = cols.iter().map(|c| c.0.as_str()).collect();
    assert_eq!(names, vec!["seq", "at", "kind", "request_id", "payload"]);

    let notnull: std::collections::HashMap<&str, i64> =
        cols.iter().map(|c| (c.0.as_str(), c.2)).collect();
    assert_eq!(notnull["at"], 1);
    assert_eq!(notnull["kind"], 1);
    assert_eq!(notnull["request_id"], 0);
    assert_eq!(notnull["payload"], 1);

    drop(store);
}
