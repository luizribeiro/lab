//! Scope §TR4a / pi-2 B-1: the two arms are *class-disjoint* — a
//! `request_id` recording is invisible to `lookup_result`, and a
//! `result_id` recording is invisible to `lookup_request`, even when the
//! underlying `JsonRpcId` values happen to collide.

use std::time::Duration;

use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::reemit::referenced_taint_index::ReferencedTaintIndex;

#[test]
fn referenced_taint_index_cross_class_lookup_returns_none() {
    let idx = ReferencedTaintIndex::new(Duration::from_secs(300));
    let shared = JsonRpcId::String("colliding-id".to_string());
    let req_taint = vec![TaintEntry {
        source: "user".to_string(),
        detail: None,
    }];
    let res_taint = vec![TaintEntry {
        source: "tool".to_string(),
        detail: Some("local/test:mock@0.1.0".to_string()),
    }];

    idx.record_request(&shared, &req_taint);
    assert_eq!(
        idx.lookup_result(&shared),
        None,
        "request-arm recording must not resolve via lookup_result"
    );

    let idx2 = ReferencedTaintIndex::new(Duration::from_secs(300));
    idx2.record_result(&shared, &res_taint);
    assert_eq!(
        idx2.lookup_request(&shared),
        None,
        "result-arm recording must not resolve via lookup_request"
    );
}
