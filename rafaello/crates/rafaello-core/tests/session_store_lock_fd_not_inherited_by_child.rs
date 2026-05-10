//! The session.lock fd is opened with `O_CLOEXEC`, so children spawned
//! after `SessionStore::open` do not inherit it (scope §S5). Uses
//! `rfl-bus-fixture probe_fd_closed` (c16) to assert the fd is closed
//! in the child.

#![cfg(feature = "test-fixture")]

use std::process::Command;

use rafaello_core::session::SessionStore;
use tempfile::TempDir;

#[test]
fn session_store_lock_fd_is_o_cloexec() {
    let dir = TempDir::new().expect("tempdir");
    let store = SessionStore::open(dir.path()).expect("open succeeds");
    let fd = store.lock_fd_for_test();

    let path = env!("CARGO_BIN_EXE_rfl-bus-fixture");
    let status = Command::new(path)
        .env_clear()
        .env("RFL_FIXTURE_MODE", "probe_fd_closed")
        .arg("--probe-fd")
        .arg(fd.to_string())
        .status()
        .expect("spawn rfl-bus-fixture");

    assert_eq!(
        status.code(),
        Some(0),
        "child should observe lock fd {} as closed (O_CLOEXEC), got status {:?}",
        fd,
        status,
    );
}
