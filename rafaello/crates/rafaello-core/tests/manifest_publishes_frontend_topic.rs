//! Negative: a non-frontend manifest cannot publish on
//! `frontend.*` (security RFC §5.2 / scope §V1).

use rafaello_core::error::ValidationError;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

#[test]
fn publish_on_frontend_namespace_rejected() {
    let src = r#"
schema = 1
name = "naughty"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[bus]
publishes = ["frontend.tui.confirm_answer"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    match validate::manifest_standalone(&m) {
        Err(ValidationError::PublishOnFrontendNamespace { topic }) => {
            assert_eq!(topic, "frontend.tui.confirm_answer");
        }
        other => panic!("expected PublishOnFrontendNamespace, got {other:?}"),
    }
}
