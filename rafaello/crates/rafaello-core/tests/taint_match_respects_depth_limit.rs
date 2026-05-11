use std::time::Duration;

use rafaello_core::bus::TaintEntry;
use rafaello_core::reemit::taint_match::TaintMatchMap;

#[test]
fn respects_depth_limit() {
    let map = TaintMatchMap::new(Duration::from_secs(60), 8);
    let taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some("<deep>".to_string()),
    }];
    let mut v = serde_json::json!("unique-deep-leaf-string");
    for _ in 0..17 {
        v = serde_json::json!({ "x": v });
    }
    map.record(&v, &taint);
    let hits = map.lookup(&serde_json::json!({"q": "unique-deep-leaf-string"}));
    assert!(
        hits.is_empty(),
        "leaf nested past MAX_WALK_DEPTH must be silently truncated; got {:?}",
        hits
    );
}
