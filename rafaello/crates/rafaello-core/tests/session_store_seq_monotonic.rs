//! `SessionStore::append_entry` returns monotonically increasing seqs and
//! `load_entries` orders by seq (scope §S1, §S2).

use rafaello_core::entry::Entry;
use rafaello_core::session::SessionStore;
use tempfile::TempDir;

#[test]
fn session_store_seq_monotonic_across_appends() {
    let dir = TempDir::new().expect("tempdir");
    let store = SessionStore::open(dir.path()).expect("open");

    let entries = [
        Entry::new_text("one"),
        Entry::new_text("two"),
        Entry::new_text("three"),
        Entry::new_text("four"),
    ];

    for (i, entry) in entries.iter().enumerate() {
        let seq = store.append_entry(entry).expect("append");
        assert_eq!(seq, i as u64);
    }

    let loaded = store.load_entries().expect("load");
    assert_eq!(loaded.len(), 4);
    for (i, stored) in loaded.iter().enumerate() {
        assert_eq!(stored.seq, i as u64);
        assert_eq!(stored.entry.id, entries[i].id);
    }
}
