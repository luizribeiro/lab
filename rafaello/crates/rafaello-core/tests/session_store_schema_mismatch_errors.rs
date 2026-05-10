//! Opening a session whose persisted `schema_version` differs from the
//! expected current version returns `SchemaMismatch` (scope §S2).

use rafaello_core::session::{SessionError, SessionStore};
use rusqlite::Connection;
use tempfile::TempDir;

#[test]
fn session_store_open_rejects_unexpected_schema_version() {
    let dir = TempDir::new().expect("tempdir");

    let store = SessionStore::open(dir.path()).expect("first open succeeds");
    drop(store);

    let conn =
        Connection::open(dir.path().join("session.sqlite")).expect("reopen sqlite for surgery");
    conn.execute(
        "UPDATE session_meta SET value = ?1 WHERE key = 'schema_version'",
        ["999"],
    )
    .expect("force schema_version");
    drop(conn);

    let err = SessionStore::open(dir.path())
        .err()
        .expect("reopen with wrong schema fails");
    match err {
        SessionError::SchemaMismatch { expected, found } => {
            assert_eq!(expected, "1");
            assert_eq!(found, "999");
        }
        other => panic!("expected SchemaMismatch, got {:?}", other),
    }
}
