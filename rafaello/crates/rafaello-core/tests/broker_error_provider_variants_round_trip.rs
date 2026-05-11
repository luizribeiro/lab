//! Acceptance for c08 (scope §B4, §B7b, pi-1 H-5): instantiate each new
//! `BrokerError` provider variant, the new `InReplyToReason::StaleRequestId`
//! variant, and every `TaintReason` variant, then exercise their
//! `Display`/`Debug` impls.

use rafaello_core::bus::JsonRpcId;
use rafaello_core::error::Publisher;
use rafaello_core::lock::CanonicalId;
use rafaello_core::{BrokerError, InReplyToReason, TaintReason};

fn provider_publisher() -> Publisher {
    let canonical = CanonicalId::parse("local/test:prov_a@0.1.0").expect("canonical");
    Publisher::Provider {
        canonical,
        provider_id: "prov_a".to_string(),
    }
}

#[test]
fn provider_variants_display_and_debug_non_panicking() {
    let canonical = CanonicalId::parse("local/test:prov_a@0.1.0").expect("canonical");

    let variants = [
        BrokerError::ProviderNotInAcl(canonical.clone()),
        BrokerError::ProviderNotRegistered(canonical.clone()),
        BrokerError::ProviderAlreadyRegistered(canonical.clone()),
        BrokerError::MissingRequestId {
            publisher: provider_publisher(),
            topic: "plugin.prov_a_local_test.tool_result".to_string(),
        },
        BrokerError::InvalidTaint {
            publisher: provider_publisher(),
            topic: "plugin.prov_a_local_test.tool_result".to_string(),
            reason: TaintReason::Missing,
        },
        BrokerError::InvalidTaint {
            publisher: provider_publisher(),
            topic: "plugin.prov_a_local_test.tool_result".to_string(),
            reason: TaintReason::EmptyArray,
        },
        BrokerError::InvalidTaint {
            publisher: provider_publisher(),
            topic: "plugin.prov_a_local_test.tool_result".to_string(),
            reason: TaintReason::UnknownSource {
                source: "unknown".to_string(),
            },
        },
    ];

    for v in &variants {
        let _ = format!("{v}");
        let _ = format!("{v:?}");
    }
}

#[test]
fn stale_request_id_display_and_debug_non_panicking() {
    let reason = InReplyToReason::StaleRequestId {
        id: JsonRpcId::from(42i64),
    };
    let _ = format!("{reason:?}");

    let err = BrokerError::InvalidInReplyTo {
        publisher: provider_publisher(),
        topic: "plugin.prov_a_local_test.tool_result".to_string(),
        reason,
    };
    let _ = format!("{err}");
    let _ = format!("{err:?}");
}
