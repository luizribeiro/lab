use std::time::Duration;

use rafaello_core::bus::TaintEntry;
use rafaello_core::reemit::taint_match::TaintMatchMap;

#[tokio::test(start_paused = true)]
async fn ttl_expires_old_entries() {
    let map = TaintMatchMap::new(Duration::from_secs(60), 8);
    let taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: None,
    }];
    map.record(&serde_json::json!({"k": "v"}), &taint);
    tokio::time::advance(Duration::from_secs(61)).await;
    let hits = map.lookup(&serde_json::json!({"k": "v"}));
    assert!(hits.is_empty());
}
