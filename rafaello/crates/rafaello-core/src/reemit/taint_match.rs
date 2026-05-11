//! Taint-match map (scope §TM1 / §TM2 / §A4).
//!
//! Records canonical-JSON byte hashes of scalar leaves observed in
//! tainted payloads, and on lookup returns the dedup'd union of taints
//! whose hash matches a scalar leaf in the lookup args. In addition,
//! string leaves whose raw UTF-8 byte length is at or above
//! `substring_min_bytes` are indexed for bidirectional substring
//! containment matching against later string leaves of equal or
//! greater length.
//!
//! Walks into nested JSON objects and arrays are bounded by
//! `MAX_WALK_DEPTH`, symmetric to the `scrubber::strip` recursion bound.
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

/// Maximum JSON nesting depth walked by `record` / `lookup`. Symmetric
/// to the `scrubber::strip` recursion bound; deeper subtrees are
/// silently truncated.
pub const MAX_WALK_DEPTH: usize = 16;

struct MapInner {
    by_hash: HashMap<u64, Vec<(Vec<TaintEntry>, Instant)>>,
    substrings: Vec<(String, Vec<TaintEntry>, Instant)>,
}

pub struct TaintMatchMap {
    entries: Mutex<MapInner>,
    ttl: Duration,
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
        let min = self.substring_min_bytes;
        let mut inner = self.entries.lock();
        walk(payload, 0, &mut |leaf| {
            let h = hash_scalar(leaf);
            inner
                .by_hash
                .entry(h)
                .or_default()
                .push((taint.to_vec(), now));
            if let serde_json::Value::String(s) = leaf {
                if s.len() >= min {
                    inner.substrings.push((s.clone(), taint.to_vec(), now));
                }
            }
        });
    }

    pub fn lookup(&self, args: &serde_json::Value) -> Vec<TaintEntry> {
        let now = Instant::now();
        let ttl = self.ttl;
        let min = self.substring_min_bytes;
        let mut inner = self.entries.lock();
        inner
            .substrings
            .retain(|(_, _, t)| now.duration_since(*t) <= ttl);
        let mut hits: Vec<TaintEntry> = Vec::new();
        walk(args, 0, &mut |leaf| {
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
            if let serde_json::Value::String(arg_s) = leaf {
                if arg_s.len() >= min {
                    for (rec_s, taints, _) in inner.substrings.iter() {
                        if rec_s.contains(arg_s.as_str()) || arg_s.contains(rec_s.as_str()) {
                            for t in taints {
                                if !hits.contains(t) {
                                    hits.push(t.clone());
                                }
                            }
                        }
                    }
                }
            }
        });
        hits
    }

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

fn walk(value: &serde_json::Value, depth: usize, on_leaf: &mut dyn FnMut(&serde_json::Value)) {
    match value {
        serde_json::Value::Object(map) => {
            if depth >= MAX_WALK_DEPTH {
                return;
            }
            for (_, v) in map {
                walk(v, depth + 1, on_leaf);
            }
        }
        serde_json::Value::Array(arr) => {
            if depth >= MAX_WALK_DEPTH {
                return;
            }
            for v in arr {
                walk(v, depth + 1, on_leaf);
            }
        }
        _ => on_leaf(value),
    }
}
