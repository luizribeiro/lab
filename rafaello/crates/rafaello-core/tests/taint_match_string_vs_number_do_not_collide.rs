use std::time::Duration;

use rafaello_core::bus::TaintEntry;
use rafaello_core::reemit::taint_match::TaintMatchMap;

#[test]
fn string_vs_number_do_not_collide() {
    let map = TaintMatchMap::new(Duration::from_secs(60), 8);
    let taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: None,
    }];
    map.record(&serde_json::json!({"n": 1}), &taint);
    let hits = map.lookup(&serde_json::json!({"n": "1"}));
    assert!(hits.is_empty());
}
