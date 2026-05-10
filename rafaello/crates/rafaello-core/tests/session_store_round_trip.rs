//! `SessionStore::append_entry` + `load_entries` round-trip across reopen
//! (scope §S1, §S2).

use rafaello_core::entry::{Entry, ToolCallStatus};
use rafaello_core::session::SessionStore;
use serde_json::json;
use tempfile::TempDir;

#[test]
fn session_store_round_trip_persists_entries_across_reopen() {
    let dir = TempDir::new().expect("tempdir");
    let state_dir = dir.path().join("state");

    let entry_a = Entry::new_text("hello world");
    let entry_b = Entry::new_code_block("fn main() {}", Some("rust"));
    let entry_c = Entry::new_tool_call(
        "call-1",
        "search",
        json!({"q": "rust"}),
        ToolCallStatus::Pending,
    );

    {
        let store = SessionStore::open(&state_dir).expect("open");
        assert_eq!(store.append_entry(&entry_a).expect("append a"), 0);
        assert_eq!(store.append_entry(&entry_b).expect("append b"), 1);
        assert_eq!(store.append_entry(&entry_c).expect("append c"), 2);
    }

    let store = SessionStore::open(&state_dir).expect("reopen");
    let loaded = store.load_entries().expect("load");

    assert_eq!(loaded.len(), 3);
    assert_eq!(loaded[0].seq, 0);
    assert_eq!(loaded[1].seq, 1);
    assert_eq!(loaded[2].seq, 2);

    assert_eq!(loaded[0].entry.id, entry_a.id);
    assert_eq!(loaded[0].entry.kind, "text");
    assert_eq!(loaded[0].entry.payload, entry_a.payload);

    assert_eq!(loaded[1].entry.id, entry_b.id);
    assert_eq!(loaded[1].entry.kind, "code_block");
    assert_eq!(loaded[1].entry.payload, entry_b.payload);

    assert_eq!(loaded[2].entry.id, entry_c.id);
    assert_eq!(loaded[2].entry.kind, "tool_call");
    assert_eq!(loaded[2].entry.payload, entry_c.payload);

    for (orig, got) in [&entry_a, &entry_b, &entry_c]
        .into_iter()
        .zip(loaded.iter())
    {
        assert_eq!(got.entry.metadata.created_at, orig.metadata.created_at);
        assert_eq!(got.entry.metadata.author, orig.metadata.author);
    }
}
