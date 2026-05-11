//! Taint-match map skeleton (scope §TM1 literal-hash half / §A4).
//!
//! Records canonical-JSON byte hashes of scalar leaves observed in
//! tainted payloads, and on lookup returns the dedup'd union of taints
//! whose hash matches a scalar leaf in the lookup args. The substring
//! arm is reserved for c06 (§TM2 substring half + bounded recursion).
//!
//! Hash key pinning: `RFL_TAINT_MATCH_HASH_KEY` is a process-restart-
//! stable pair fed to `SipHasher13` so test reproducibility doesn't
//! depend on `DefaultHasher`'s per-process randomisation. The map is
//! in-process only — the determinism is for tests, not for any wire
//! invariant.

use std::collections::HashMap;
use std::hash::Hasher;
use std::time::Duration;

use tokio::time::Instant;

use parking_lot::Mutex;
use siphasher::sip::SipHasher13;

use crate::bus::TaintEntry;

/// Pinned SipHasher13 keys (k0, k1) used by the literal-hash arm.
#[allow(clippy::unusual_byte_groupings)]
pub const RFL_TAINT_MATCH_HASH_KEY: (u64, u64) =
    (0xc0ffee_d00d_f00d_b002_u128 as u64, 0xa11ce_b0b_face_b00c);

struct MapInner {
    by_hash: HashMap<u64, Vec<(Vec<TaintEntry>, Instant)>>,
    #[allow(dead_code)]
    substrings: Vec<(String, Vec<TaintEntry>, Instant)>,
}

pub struct TaintMatchMap {
    entries: Mutex<MapInner>,
    ttl: Duration,
    #[allow(dead_code)]
    substring_min_bytes: usize,
}

impl TaintMatchMap {
    pub fn new(ttl: Duration, substring_min_bytes: usize) -> Self {
        Self {
            entries: Mutex::new(MapInner {
                by_hash: HashMap::new(),
                substrings: Vec::new(),
            }),
            ttl,
            substring_min_bytes,
        }
    }

    pub fn record(&self, payload: &serde_json::Value, taint: &[TaintEntry]) {
        let now = Instant::now();
        let mut inner = self.entries.lock();
        for_each_scalar_leaf(payload, &mut |leaf| {
            let h = hash_scalar(leaf);
            inner
                .by_hash
                .entry(h)
                .or_default()
                .push((taint.to_vec(), now));
        });
    }

    pub fn lookup(&self, args: &serde_json::Value) -> Vec<TaintEntry> {
        let now = Instant::now();
        let ttl = self.ttl;
        let mut inner = self.entries.lock();
        let mut hits: Vec<TaintEntry> = Vec::new();
        for_each_scalar_leaf(args, &mut |leaf| {
            let h = hash_scalar(leaf);
            if let Some(bucket) = inner.by_hash.get_mut(&h) {
                bucket.retain(|(_, t)| now.duration_since(*t) <= ttl);
                for (taints, _) in bucket.iter() {
                    for t in taints {
                        if !hits.contains(t) {
                            hits.push(t.clone());
                        }
                    }
                }
                if bucket.is_empty() {
                    inner.by_hash.remove(&h);
                }
            }
        });
        hits
    }

    #[allow(dead_code)]
    // consumer lands in c08 per m5b commits.md
    pub fn clear(&self) {
        let mut inner = self.entries.lock();
        inner.by_hash.clear();
        inner.substrings.clear();
    }
}

fn hash_scalar(value: &serde_json::Value) -> u64 {
    let bytes = serde_json::to_vec(value).expect("serde_json scalar encoding is infallible");
    let (k0, k1) = RFL_TAINT_MATCH_HASH_KEY;
    let mut hasher = SipHasher13::new_with_keys(k0, k1);
    hasher.write(&bytes);
    hasher.finish()
}

fn for_each_scalar_leaf(value: &serde_json::Value, f: &mut dyn FnMut(&serde_json::Value)) {
    match value {
        serde_json::Value::Object(map) => {
            for (_, v) in map {
                if v.is_object() || v.is_array() {
                    continue;
                }
                f(v);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                if v.is_object() || v.is_array() {
                    continue;
                }
                f(v);
            }
        }
        _ => f(value),
    }
}
