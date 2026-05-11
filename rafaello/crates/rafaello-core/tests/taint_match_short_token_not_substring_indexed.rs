use std::time::Duration;

use rafaello_core::bus::TaintEntry;
use rafaello_core::reemit::taint_match::TaintMatchMap;

#[test]
fn short_token_not_substring_indexed() {
    let map = TaintMatchMap::new(Duration::from_secs(60), 16);
    let taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some("<short>".to_string()),
    }];
    map.record(&serde_json::json!({"ack": "ok"}), &taint);
    let hits = map.lookup(&serde_json::json!({"reply": "please say ok now"}));
    assert!(
        hits.is_empty(),
        "below-threshold recorded string must not match as a substring; got {:?}",
        hits
    );
}
