//! Acceptance for m5b c01 (scope §PT3 / §A1): the new
//! `BrokerError::TaintSupersetViolated` variant constructs with a non-empty
//! `missing` vector and renders through both the `thiserror`-derived `Display`
//! impl and the `Debug` impl.

use rafaello_core::bus::TaintEntry;
use rafaello_core::error::Publisher;
use rafaello_core::lock::CanonicalId;
use rafaello_core::BrokerError;

fn provider_publisher() -> Publisher {
    let canonical = CanonicalId::parse("local/test:prov_a@0.1.0").expect("canonical");
    Publisher::Provider {
        canonical,
        provider_id: "prov_a".to_string(),
    }
}

#[test]
fn taint_superset_violated_display_includes_publisher_topic_and_missing() {
    let missing = vec![
        TaintEntry {
            source: "tool:fs.read".to_string(),
            detail: Some("/etc/secret".to_string()),
        },
        TaintEntry {
            source: "user".to_string(),
            detail: None,
        },
    ];
    let err = BrokerError::TaintSupersetViolated {
        publisher: provider_publisher(),
        topic: "plugin.prov_a_local_test.tool_result".to_string(),
        missing,
    };

    let rendered = format!("{err}");
    assert!(
        rendered.contains("plugin.prov_a_local_test.tool_result"),
        "Display should mention the topic, got: {rendered}"
    );
    assert!(
        rendered.contains("not a superset of in_reply_to ancestry"),
        "Display should mention the contradiction, got: {rendered}"
    );
    assert!(
        rendered.contains("missing entries:"),
        "Display should list the missing entries, got: {rendered}"
    );
    assert!(
        rendered.contains("tool:fs.read"),
        "Display should embed the missing TaintEntry source, got: {rendered}"
    );

    let debug = format!("{err:?}");
    assert!(
        debug.contains("TaintSupersetViolated"),
        "Debug should name the variant, got: {debug}"
    );
    assert!(
        debug.contains("tool:fs.read"),
        "Debug should embed the missing TaintEntry source, got: {debug}"
    );
}
