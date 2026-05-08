//! Negative: a manifest with `provides.provider = "anthropic"` that
//! publishes on `provider.openai.*` is rejected by
//! `validate::manifest_with_id` (scope §V2).

use rafaello_core::error::ValidationError;
use rafaello_core::lock::CanonicalId;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

#[test]
fn provider_namespace_mismatch_rejected() {
    let src = r#"
schema = 1
name = "anthropic-llm"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[provides]
provider = "anthropic"

[bus]
publishes = ["provider.openai.tool_request"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    let canonical = CanonicalId::parse("github/acme:anthropic-llm@0.1.0").unwrap();
    match validate::manifest_with_id(&m, &canonical) {
        Err(ValidationError::ProviderNamespaceMismatch) => {}
        other => panic!("expected ProviderNamespaceMismatch, got {other:?}"),
    }
}
