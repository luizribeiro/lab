//! Scope §TR4a / §A10: a lookup miss returns `None`, which downstream
//! consumers treat as fail-open empty.

use std::time::Duration;

use rafaello_core::bus::JsonRpcId;
use rafaello_core::reemit::referenced_taint_index::ReferencedTaintIndex;

#[test]
fn referenced_taint_index_lookup_miss_returns_none() {
    let idx = ReferencedTaintIndex::new(Duration::from_secs(300));
    let id = JsonRpcId::String("never-recorded".to_string());

    assert_eq!(idx.lookup_request(&id), None);
    assert_eq!(idx.lookup_result(&id), None);
}
