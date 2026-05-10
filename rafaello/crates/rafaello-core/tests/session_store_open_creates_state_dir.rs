//! `SessionStore::open` creates the state directory and initializes
//! lockfile + sqlite (scope §S1, §S2, §S5).

use rafaello_core::session::SessionStore;
use tempfile::TempDir;

#[test]
fn session_store_open_creates_state_dir_and_files() {
    let parent = TempDir::new().expect("tempdir");
    let state_dir = parent.path().join("nested").join("state");
    assert!(!state_dir.exists());

    let store = SessionStore::open(&state_dir).expect("open succeeds");

    assert!(state_dir.is_dir());
    assert!(state_dir.join("session.lock").is_file());
    assert!(state_dir.join("session.sqlite").is_file());

    let session_id = store.session_id().to_string();
    assert_eq!(
        session_id.len(),
        26,
        "session_id should be a 26-char ULID, got {:?}",
        session_id,
    );

    let lock_contents =
        std::fs::read_to_string(state_dir.join("session.lock")).expect("read lockfile");
    let pid: u32 = lock_contents
        .trim()
        .parse()
        .expect("lockfile holds u32 pid");
    assert_eq!(pid, std::process::id());
}
