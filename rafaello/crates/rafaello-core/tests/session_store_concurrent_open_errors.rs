//! A second `SessionStore::open` on the same state_dir while the first is
//! still alive returns `SessionError::Locked` with the holding pid
//! (scope §S5).

use rafaello_core::session::{SessionError, SessionStore};
use tempfile::TempDir;

#[test]
fn session_store_second_open_returns_locked_with_holder_pid() {
    let dir = TempDir::new().expect("tempdir");
    let _first = SessionStore::open(dir.path()).expect("first open succeeds");

    let err = SessionStore::open(dir.path())
        .err()
        .expect("second open fails");
    match err {
        SessionError::Locked { holder_pid } => {
            assert_eq!(
                holder_pid,
                Some(std::process::id()),
                "expected holder pid {} got {:?}",
                std::process::id(),
                holder_pid,
            );
        }
        other => panic!("expected Locked, got {:?}", other),
    }
}
