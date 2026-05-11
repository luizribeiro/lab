use std::time::Duration;

use rafaello_core::bus::TaintEntry;
use rafaello_core::reemit::taint_match::TaintMatchMap;

#[test]
fn substring_handles_backslash_escape() {
    let map = TaintMatchMap::new(Duration::from_secs(60), 8);
    let taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some("<path>".to_string()),
    }];
    map.record(&serde_json::json!({"p": "path\\to\\file.txt"}), &taint);
    let hits = map.lookup(&serde_json::json!({"q": "to\\file.txt"}));
    assert_eq!(hits, taint);
}
