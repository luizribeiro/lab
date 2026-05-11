use std::time::Duration;

use rafaello_core::bus::TaintEntry;
use rafaello_core::reemit::taint_match::TaintMatchMap;

#[test]
fn substring_only_strings() {
    let map = TaintMatchMap::new(Duration::from_secs(60), 8);
    let taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some("<port>".to_string()),
    }];
    map.record(&serde_json::json!({"port": 8443}), &taint);
    let hits = map.lookup(&serde_json::json!({"host": "hostname-8443.example.com"}));
    assert!(
        hits.is_empty(),
        "non-string scalar must not substring-index; got {:?}",
        hits
    );
}
