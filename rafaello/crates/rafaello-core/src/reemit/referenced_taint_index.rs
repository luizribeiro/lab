//! Referenced-taint index (scope §TR4a / pi-2 B-1).
//!
//! Two disjoint id-keyed caches that record the taint observed alongside
//! a canonical request id and (separately) a tool-result id. The two
//! arms are *class-disjoint* per pi-2 B-1: a `request_id` lookup never
//! resolves a `result_id` record and vice versa, even if the underlying
//! `JsonRpcId` values happen to collide.
//!
//! Both arms share the same TTL (default 5 min, wired through
//! `ReemitRouter`). Expiry is lazy: each `record` / `lookup` evicts
//! stale entries from the relevant arm before reading or returning.
//!
//! Lookup-miss semantics (scope §A10): a miss returns `None`, which
//! consumers treat as fail-open empty.

use std::collections::HashMap;
use std::time::Duration;

use parking_lot::Mutex;
use tokio::time::Instant;

use crate::bus::{JsonRpcId, TaintEntry};

pub struct ReferencedTaintIndex {
    by_request_id: Mutex<HashMap<JsonRpcId, (Vec<TaintEntry>, Instant)>>,
    by_result_id: Mutex<HashMap<JsonRpcId, (Vec<TaintEntry>, Instant)>>,
    ttl: Duration,
}

impl ReferencedTaintIndex {
    pub fn new(ttl: Duration) -> Self {
        Self {
            by_request_id: Mutex::new(HashMap::new()),
            by_result_id: Mutex::new(HashMap::new()),
            ttl,
        }
    }

    pub fn record_request(&self, request_id: &JsonRpcId, taint: &[TaintEntry]) {
        let now = Instant::now();
        let ttl = self.ttl;
        let mut map = self.by_request_id.lock();
        map.retain(|_, (_, t)| now.duration_since(*t) <= ttl);
        map.insert(request_id.clone(), (taint.to_vec(), now));
    }

    pub fn record_result(&self, result_id: &JsonRpcId, taint: &[TaintEntry]) {
        let now = Instant::now();
        let ttl = self.ttl;
        let mut map = self.by_result_id.lock();
        map.retain(|_, (_, t)| now.duration_since(*t) <= ttl);
        map.insert(result_id.clone(), (taint.to_vec(), now));
    }

    pub fn lookup_request(&self, request_id: &JsonRpcId) -> Option<Vec<TaintEntry>> {
        let now = Instant::now();
        let ttl = self.ttl;
        let mut map = self.by_request_id.lock();
        map.retain(|_, (_, t)| now.duration_since(*t) <= ttl);
        map.get(request_id).map(|(taint, _)| taint.clone())
    }

    pub fn lookup_result(&self, result_id: &JsonRpcId) -> Option<Vec<TaintEntry>> {
        let now = Instant::now();
        let ttl = self.ttl;
        let mut map = self.by_result_id.lock();
        map.retain(|_, (_, t)| now.duration_since(*t) <= ttl);
        map.get(result_id).map(|(taint, _)| taint.clone())
    }

    pub fn clear(&self) {
        self.by_request_id.lock().clear();
        self.by_result_id.lock().clear();
    }
}
