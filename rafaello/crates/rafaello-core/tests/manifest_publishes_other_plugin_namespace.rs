//! Negative: a manifest publishing on `plugin.<topic-id>.*` whose
//! `<topic-id>` does not match `topic_id::derive(canonical)` is
//! rejected by `validate::manifest_with_id` (scope §V2).

use rafaello_core::error::ValidationError;
use rafaello_core::lock::CanonicalId;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

#[test]
fn publishes_other_plugin_namespace_rejected() {
    let src = r#"
schema = 1
name = "naughty"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[bus]
publishes = ["plugin.id_aaaaaaaaaaaaaaaa.foo"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    let canonical = CanonicalId::parse("github/acme:naughty@0.1.0").unwrap();
    match validate::manifest_with_id(&m, &canonical) {
        Err(ValidationError::PublishOnForeignTopicId) => {}
        other => panic!("expected PublishOnForeignTopicId, got {other:?}"),
    }
}
