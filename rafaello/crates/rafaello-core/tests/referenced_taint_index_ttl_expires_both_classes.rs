//! Scope §TR4a / pi-2 B-1: both arms share the same TTL. After the TTL
//! elapses, entries in either arm must be evicted on the next lookup.

use std::time::Duration;

use rafaello_core::bus::{JsonRpcId, TaintEntry};
use rafaello_core::reemit::referenced_taint_index::ReferencedTaintIndex;

#[tokio::test(start_paused = true)]
async fn referenced_taint_index_ttl_expires_both_classes() {
    let idx = ReferencedTaintIndex::new(Duration::from_secs(60));
    let req_id = JsonRpcId::String("req-1".to_string());
    let res_id = JsonRpcId::String("res-1".to_string());
    let taint = vec![TaintEntry {
        source: "user".to_string(),
        detail: None,
    }];

    idx.record_request(&req_id, &taint);
    idx.record_result(&res_id, &taint);

    tokio::time::advance(Duration::from_secs(30)).await;
    assert_eq!(idx.lookup_request(&req_id), Some(taint.clone()));
    assert_eq!(idx.lookup_result(&res_id), Some(taint.clone()));

    tokio::time::advance(Duration::from_secs(31)).await;
    assert_eq!(
        idx.lookup_request(&req_id),
        None,
        "request arm must expire past TTL",
    );
    assert_eq!(
        idx.lookup_result(&res_id),
        None,
        "result arm must expire past TTL",
    );
}
