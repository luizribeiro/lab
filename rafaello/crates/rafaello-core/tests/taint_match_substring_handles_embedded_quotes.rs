use std::time::Duration;

use rafaello_core::bus::TaintEntry;
use rafaello_core::reemit::taint_match::TaintMatchMap;

#[test]
fn substring_handles_embedded_quotes() {
    let map = TaintMatchMap::new(Duration::from_secs(60), 8);
    let taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some("<mail>".to_string()),
    }];
    map.record(
        &serde_json::json!({"msg": "please email \"alice\"@example.com"}),
        &taint,
    );
    let hits = map.lookup(&serde_json::json!({"to": "\"alice\"@example.com"}));
    assert_eq!(hits, taint);
}
