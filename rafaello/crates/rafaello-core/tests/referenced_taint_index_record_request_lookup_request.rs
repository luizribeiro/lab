//! Scope §TR4a / pi-2 B-1: `record_request` followed by
//! `lookup_request` for the same `JsonRpcId` returns the recorded taint.

use std::time::Duration;

use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::reemit::referenced_taint_index::ReferencedTaintIndex;

#[test]
fn referenced_taint_index_record_request_lookup_request() {
    let idx = ReferencedTaintIndex::new(Duration::from_secs(300));
    let id = JsonRpcId::String("req-1".to_string());
    let taint = vec![TaintEntry {
        source: "user".to_string(),
        detail: None,
    }];

    idx.record_request(&id, &taint);

    let hits = idx.lookup_request(&id);
    assert_eq!(hits, Some(taint));
}
