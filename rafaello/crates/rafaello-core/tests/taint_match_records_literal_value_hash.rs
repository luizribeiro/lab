use std::time::Duration;

use rafaello_core::bus::TaintEntry;
use rafaello_core::reemit::taint_match::TaintMatchMap;

#[test]
fn records_literal_value_hash() {
    let map = TaintMatchMap::new(Duration::from_secs(60), 8);
    let taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some("<fetch>".to_string()),
    }];
    map.record(&serde_json::json!({"token": "X-token-here"}), &taint);
    let hits = map.lookup(&serde_json::json!({"url": "X-token-here"}));
    assert_eq!(hits, taint);
}
