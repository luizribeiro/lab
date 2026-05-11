//! Scope §TR4a / pi-2 B-1: `clear()` drops entries from both arms.

use std::time::Duration;

use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::reemit::referenced_taint_index::ReferencedTaintIndex;

#[test]
fn referenced_taint_index_clear_drops_both_classes() {
    let idx = ReferencedTaintIndex::new(Duration::from_secs(300));
    let req_id = JsonRpcId::String("req-1".to_string());
    let res_id = JsonRpcId::String("res-1".to_string());
    let taint = vec![TaintEntry {
        source: "user".to_string(),
        detail: None,
    }];

    idx.record_request(&req_id, &taint);
    idx.record_result(&res_id, &taint);
    assert_eq!(idx.lookup_request(&req_id), Some(taint.clone()));
    assert_eq!(idx.lookup_result(&res_id), Some(taint.clone()));

    idx.clear();

    assert_eq!(idx.lookup_request(&req_id), None);
    assert_eq!(idx.lookup_result(&res_id), None);
}
