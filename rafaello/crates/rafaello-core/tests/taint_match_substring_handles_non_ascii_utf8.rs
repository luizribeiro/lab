use std::time::Duration;

use rafaello_core::bus::TaintEntry;
use rafaello_core::reemit::taint_match::TaintMatchMap;

#[test]
fn substring_handles_non_ascii_utf8() {
    let map = TaintMatchMap::new(Duration::from_secs(60), 8);
    let taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some("<jp>".to_string()),
    }];
    let recorded = "日本語の長い文字列の途中にあるURL";
    let arg = "日本語の長い文字列";
    assert!(recorded.len() >= 8);
    assert!(arg.len() >= 8);
    map.record(&serde_json::json!({"v": recorded}), &taint);
    let hits = map.lookup(&serde_json::json!({"q": arg}));
    assert_eq!(hits, taint);
}
