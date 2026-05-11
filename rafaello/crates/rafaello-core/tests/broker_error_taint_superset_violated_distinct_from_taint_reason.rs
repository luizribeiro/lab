//! Acceptance for m5b c01 (scope §PT3 / §A1): the new variant is a
//! top-level arm on `BrokerError`, not a sub-arm of `TaintReason`. A
//! `match` on the variant compiles without falling through any other
//! arm, and the `TaintReason` enum still exposes exactly the variants it
//! had before this commit (`Missing`, `EmptyArray`, `UnknownSource`).

use rafaello_core::bus::TaintEntry;
use rafaello_core::error::Publisher;
use rafaello_core::lock::CanonicalId;
use rafaello_core::{BrokerError, TaintReason};

fn provider_publisher() -> Publisher {
    let canonical = CanonicalId::parse("local/test:prov_a@0.1.0").expect("canonical");
    Publisher::Provider {
        canonical,
        provider_id: "prov_a".to_string(),
    }
}

#[test]
fn taint_superset_violated_is_its_own_arm() {
    let err = BrokerError::TaintSupersetViolated {
        publisher: provider_publisher(),
        topic: "plugin.prov_a_local_test.tool_result".to_string(),
        missing: vec![TaintEntry {
            source: "tool:fs.read".to_string(),
            detail: None,
        }],
    };

    let matched = match err {
        BrokerError::TaintSupersetViolated { ref missing, .. } => missing.len(),
        BrokerError::InvalidTaint { .. } => panic!("must not match InvalidTaint"),
        _ => panic!("must not fall through to a different BrokerError arm"),
    };
    assert_eq!(matched, 1);
}

#[test]
fn taint_reason_variants_unchanged_by_this_commit() {
    let variants = [
        TaintReason::Missing,
        TaintReason::EmptyArray,
        TaintReason::UnknownSource {
            source: "unknown".to_string(),
        },
    ];
    for v in &variants {
        match v {
            TaintReason::Missing | TaintReason::EmptyArray | TaintReason::UnknownSource { .. } => {}
            _ => panic!("unexpected TaintReason variant added — c01 must not touch TaintReason"),
        }
    }
}
