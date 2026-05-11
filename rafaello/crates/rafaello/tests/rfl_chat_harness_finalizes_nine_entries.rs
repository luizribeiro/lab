mod common;

use std::process::Command;

use common::m4_lock_fixture::write_stub_lock;
use common::workspace_bin_path::workspace_bin;
use rusqlite::Connection;

#[test]
fn harness_finalizes_nine_entries() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");

    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path();
    write_stub_lock(project_root);

    let output = Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(project_root)
        .env("RFL_HARNESS_FIXTURES", "1")
        .env("RFL_TUI_TEST_MODE", "1")
        .env("RFL_TUI_PATH", workspace_bin("rfl-tui"))
        .env("RFL_TUI_MAX_LIFETIME", "2")
        .output()
        .expect("spawn rfl chat");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected zero exit; stderr={stderr}"
    );

    let db_path = project_root
        .join(".rafaello")
        .join("state")
        .join("session.sqlite");
    assert!(db_path.is_file(), "session.sqlite missing at {db_path:?}");

    let conn = Connection::open(&db_path).expect("open sqlite");
    let mut stmt = conn
        .prepare("SELECT kind FROM entries ORDER BY seq ASC")
        .expect("prepare");
    let kinds: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .expect("query")
        .map(|r| r.expect("row"))
        .collect();

    assert_eq!(
        kinds,
        vec![
            "text",
            "heading",
            "code_block",
            "thinking",
            "tool_call",
            "tool_result",
            "image",
            "error",
            "myorg:custom",
        ],
        "unexpected entry kinds; stderr={stderr}"
    );
}
