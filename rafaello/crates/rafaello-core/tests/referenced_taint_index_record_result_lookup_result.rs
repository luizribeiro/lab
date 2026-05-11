//! Scope §TR4a / pi-2 B-1: `record_result` followed by `lookup_result`
//! for the same `JsonRpcId` returns the recorded taint.

use std::time::Duration;

use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::reemit::referenced_taint_index::ReferencedTaintIndex;

#[test]
fn referenced_taint_index_record_result_lookup_result() {
    let idx = ReferencedTaintIndex::new(Duration::from_secs(300));
    let id = JsonRpcId::String("result-7".to_string());
    let taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some("local/test:mock@0.1.0".to_string()),
    }];

    idx.record_result(&id, &taint);

    let hits = idx.lookup_result(&id);
    assert_eq!(hits, Some(taint));
}
