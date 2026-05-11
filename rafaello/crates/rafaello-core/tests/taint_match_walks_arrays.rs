use std::time::Duration;

use rafaello_core::bus::TaintEntry;
use rafaello_core::reemit::taint_match::TaintMatchMap;

#[test]
fn walks_arrays() {
    let map = TaintMatchMap::new(Duration::from_secs(60), 8);
    let taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some("<arr>".to_string()),
    }];
    map.record(
        &serde_json::json!({"items": ["alpha-token-here", "beta-token-here"]}),
        &taint,
    );
    let hit_beta = map.lookup(&serde_json::json!({"x": "beta-token-here"}));
    assert_eq!(hit_beta, taint);
    let hit_gamma = map.lookup(&serde_json::json!({"x": "gamma-token-here"}));
    assert!(
        hit_gamma.is_empty(),
        "unrelated string must not match; got {:?}",
        hit_gamma
    );
}
