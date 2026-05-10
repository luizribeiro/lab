//! A locked session.lock with empty / corrupt content yields
//! `Locked { holder_pid: None }` rather than spuriously parsing
//! garbage as a pid (scope §S5).

use std::fs::OpenOptions;
use std::io::Write;

use nix::fcntl::{Flock, FlockArg};

use rafaello_core::session::{SessionError, SessionStore};
use tempfile::TempDir;

fn try_open_expect_unknown(state_dir: &std::path::Path) {
    let err = SessionStore::open(state_dir).err().expect("open fails");
    match err {
        SessionError::Locked { holder_pid: None } => {}
        SessionError::Locked { holder_pid } => {
            panic!("expected holder_pid: None, got {:?}", holder_pid)
        }
        other => panic!("expected Locked, got {:?}", other),
    }
}

#[test]
fn session_store_locked_with_empty_lockfile_returns_unknown() {
    let dir = TempDir::new().expect("tempdir");
    let lock_path = dir.path().join("session.lock");

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)
        .expect("create lockfile");
    let _guard = Flock::lock(file, FlockArg::LockExclusiveNonblock)
        .expect("acquire flock on empty lockfile");

    try_open_expect_unknown(dir.path());
}

#[test]
fn session_store_locked_with_corrupt_lockfile_returns_unknown() {
    let dir = TempDir::new().expect("tempdir");
    let lock_path = dir.path().join("session.lock");

    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&lock_path)
        .expect("create lockfile");
    file.write_all(b"not-a-pid\n").expect("write garbage");
    file.sync_all().expect("sync");

    let _guard = Flock::lock(file, FlockArg::LockExclusiveNonblock)
        .expect("acquire flock on corrupt lockfile");

    try_open_expect_unknown(dir.path());
}
