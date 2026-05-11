//! c27 — headline demo bar (scope §I): drive `rfl chat` with
//! `RFL_TUI_TEST_MESSAGE="what's in README.md"`, watch the mockprovider +
//! readfile round-trip end-to-end, and pin both the canonical [`Entry`]
//! shape persisted in `session.sqlite` (pi-1 B-8) AND the four
//! `core.session.entry.finalized` events forwarded on the TUI's stderr
//! (pi-2 H-4 + pi-3 B-4) — including the assistant message's exact
//! formatted text.

mod common;

use std::process::Command;

use common::m4_lock_fixture::write_stub_lock;
use common::workspace_bin_path::workspace_bin;
use rafaello_core::entry::payloads::TextPayload;
use rafaello_core::entry::EntryAuthor;
use rafaello_core::session::SessionStore;
use serial_test::serial;

const README_BODY: &str = "m4 demo readme\n";

#[test]
#[serial(rfl_chat)]
fn rfl_chat_demo_bar_read_file() {
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
        "expected zero exit; stderr={stderr}"
    );

    let finalized_lines: Vec<&str> = stderr
        .lines()
        .filter(|l| l.contains("rfl-tui: bus.event topic=core.session.entry.finalized"))
        .collect();
    assert_eq!(
        finalized_lines.len(),
        4,
        "expected four forwarded `core.session.entry.finalized` sentinels on TUI stderr; \
         got {:#?}\nfull stderr:\n{stderr}",
        finalized_lines
    );

    let state_dir = project_root.join(".rafaello").join("state");
    let store = SessionStore::open(&state_dir).expect("open SessionStore after rfl exit");
    let stored = store.load_entries().expect("load entries");

    assert_eq!(
        stored.len(),
        4,
        "expected exactly four persisted entries; got {:#?}",
        stored
    );

    let expectations: [(u64, &str, EntryAuthor); 4] = [
        (0, "text", EntryAuthor::User),
        (1, "tool_call", EntryAuthor::Assistant),
        (2, "tool_result", EntryAuthor::Tool),
        (3, "text", EntryAuthor::Assistant),
    ];
    for (i, (seq, kind, author)) in expectations.iter().enumerate() {
        let row = &stored[i];
        assert_eq!(row.seq, *seq, "row {i}: unexpected seq");
        assert_eq!(row.entry.kind, *kind, "row {i}: unexpected kind");
        assert_eq!(
            row.entry.metadata.author, *author,
            "row {i}: unexpected author"
        );
    }

    let user_text: TextPayload =
        serde_json::from_value(stored[0].entry.payload.clone()).expect("decode user TextPayload");
    assert_eq!(user_text.text, "what's in README.md");

    let assistant_text: TextPayload = serde_json::from_value(stored[3].entry.payload.clone())
        .expect("decode assistant TextPayload");
    assert_eq!(
        assistant_text.text,
        "Here's what's in README.md:\nm4 demo readme\n"
    );
}
