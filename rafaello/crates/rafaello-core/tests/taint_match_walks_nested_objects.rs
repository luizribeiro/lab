use std::time::Duration;

use rafaello_core::bus::TaintEntry;
use rafaello_core::reemit::taint_match::TaintMatchMap;

#[test]
fn walks_nested_objects() {
    let map = TaintMatchMap::new(Duration::from_secs(60), 8);
    let taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some("<nest>".to_string()),
    }];
    map.record(
        &serde_json::json!({
            "outer": { "inner": { "token": "verbatim-string-here" } }
        }),
        &taint,
    );
    let hits = map.lookup(&serde_json::json!({"ref": "verbatim-string-here"}));
    assert_eq!(hits, taint);
}
