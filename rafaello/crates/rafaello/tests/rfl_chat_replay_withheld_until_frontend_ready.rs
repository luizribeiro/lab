mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::entry::Entry;
use rafaello_core::session::SessionStore;

#[test]
fn replay_withheld_until_frontend_ready() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");

    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path();

    let state_dir = project_root.join(".rafaello").join("state");
    {
        let store = SessionStore::open(&state_dir).expect("open SessionStore");
        store
            .append_entry(&Entry::new_text("seeded entry"))
            .expect("append entry");
    }

    let output = Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(project_root)
        .env("RFL_TUI_PATH", workspace_bin("rfl-tui"))
        .env("RFL_TUI_TEST_MODE", "1")
        .env("RFL_TUI_MAX_LIFETIME", "2")
        .output()
        .expect("spawn rfl chat");

    let stderr = String::from_utf8_lossy(&output.stderr);

    let project_root_idx = stderr
        .find("rfl-tui: project-root=")
        .unwrap_or_else(|| panic!("missing project-root sentinel: {stderr}"));
    let ready_idx = stderr
        .find("rfl-chat: frontend-ready-observed")
        .unwrap_or_else(|| panic!("missing frontend-ready-observed: {stderr}"));
    let replay_idx = stderr
        .find("rfl-tui: bus.event topic=core.session.entry.finalized")
        .unwrap_or_else(|| panic!("missing replay event: {stderr}"));

    assert!(
        project_root_idx < ready_idx,
        "project-root must precede ready sentinel: {stderr}"
    );
    assert!(
        ready_idx < replay_idx,
        "ready sentinel must precede replay event: {stderr}"
    );
}
