use std::time::Duration;

use rafaello_core::bus::TaintEntry;
use rafaello_core::reemit::taint_match::TaintMatchMap;

#[test]
fn records_numbers_via_literal_hash() {
    let map = TaintMatchMap::new(Duration::from_secs(60), 8);
    let taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some("port-source".to_string()),
    }];
    map.record(&serde_json::json!({"port": 8443}), &taint);
    let hits = map.lookup(&serde_json::json!({"port": 8443}));
    assert_eq!(hits, taint);
}
