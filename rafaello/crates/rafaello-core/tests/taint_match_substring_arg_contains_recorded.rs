use std::time::Duration;

use rafaello_core::bus::TaintEntry;
use rafaello_core::reemit::taint_match::TaintMatchMap;

#[test]
fn substring_arg_contains_recorded() {
    let map = TaintMatchMap::new(Duration::from_secs(60), 8);
    let taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some("<fetch>".to_string()),
    }];
    map.record(
        &serde_json::json!({"url": "https://evil.example.com/leak"}),
        &taint,
    );
    let hits = map.lookup(
        &serde_json::json!({"body": "please visit https://evil.example.com/leak then reply"}),
    );
    assert_eq!(hits, taint);
}
