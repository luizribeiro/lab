//! Topic-id derivation and collision detection (scope §T1–§T3).
//!
//! `derive` is the only entry point that hashes; `collisions_with_prefixes`
//! is a public stable helper so integration tests can synthesise collisions
//! without a feature-gated test seam (pi review-2 finding 8).

use data_encoding::BASE32_NOPAD;
use sha2::{Digest, Sha256};

use crate::error::CollisionError;
use crate::lock::CanonicalId;

const PREFIX_BYTES: usize = 10;

pub fn derive(canonical_id: &str) -> String {
    let digest = Sha256::digest(canonical_id.as_bytes());
    let encoded = BASE32_NOPAD.encode(&digest[..PREFIX_BYTES]);
    let mut out = String::with_capacity(3 + encoded.len());
    out.push_str("id_");
    for ch in encoded.chars() {
        out.push(ch.to_ascii_lowercase());
    }
    out
}

pub fn collisions_with_prefixes(pairs: &[(CanonicalId, String)]) -> Result<(), CollisionError> {
    for i in 0..pairs.len() {
        for j in (i + 1)..pairs.len() {
            let (a, prefix_a) = &pairs[i];
            let (b, prefix_b) = &pairs[j];
            if prefix_a == prefix_b && a != b {
                return Err(CollisionError::TopicIdCollision {
                    a: a.to_string(),
                    b: b.to_string(),
                    prefix: prefix_a.clone(),
                });
            }
        }
    }
    Ok(())
}

pub fn collisions(plugins: &[CanonicalId]) -> Result<(), CollisionError> {
    let pairs: Vec<(CanonicalId, String)> = plugins
        .iter()
        .map(|p| (p.clone(), derive(&p.to_string())))
        .collect();
    collisions_with_prefixes(&pairs)
}
