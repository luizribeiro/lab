//! c31 / pi-2 B-2 regression — load an m4-shaped readfile lock and
//! assert `ToolSchemaCatalog::build` succeeds at `run_chat` startup
//! (the new catalog-cutover step), `PluginSupervisor::new` accepts
//! the catalog, and the existing m4 demo flow still drives a
//! `read-file` tool call end-to-end.

mod common;

use std::process::Command;

use common::m4_lock_fixture::write_stub_lock;
use common::workspace_bin_path::workspace_bin;
use rafaello_core::session::SessionStore;
use serial_test::serial;

const README_BODY: &str = "m4 demo readme\n";

#[test]
#[serial(rfl_chat)]
fn rfl_chat_existing_m4_readfile_demo_still_starts() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");
    let _ = workspace_bin("rfl-mockprovider");
    let _ = workspace_bin("rfl-readfile");

    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path();
    std::fs::write(project_root.join("README.md"), README_BODY).expect("write README.md");
    write_stub_lock(project_root);

    let output = Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(project_root)
        .env("RFL_TUI_TEST_MODE", "1")
        .env("RFL_TUI_PATH", workspace_bin("rfl-tui"))
        .env("RFL_TUI_TEST_MESSAGE", "what's in README.md")
        .env("RFL_TUI_MAX_LIFETIME", "5")
        .output()
        .expect("spawn rfl chat");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected zero exit after catalog cutover; stderr={stderr}"
    );

    let state_dir = project_root.join(".rafaello").join("state");
    let store = SessionStore::open(&state_dir).expect("open SessionStore after rfl exit");
    let stored = store.load_entries().expect("load entries");
    assert_eq!(
        stored.len(),
        4,
        "expected four persisted entries (user, tool_call, tool_result, assistant); got {:#?}",
        stored
    );
    assert_eq!(stored[1].entry.kind, "tool_call", "expected a tool_call");
    assert_eq!(
        stored[2].entry.kind, "tool_result",
        "expected a tool_result"
    );
}
