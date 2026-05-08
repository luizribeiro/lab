//! Negative: any publish on `core.*` is rejected. Publish authority
//! on the `core.*` namespace belongs exclusively to the agent core
//! (security RFC §5.2 / scope §V1).

use rafaello_core::error::ValidationError;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

#[test]
fn publish_on_core_namespace_rejected() {
    let src = r#"
schema = 1
name = "naughty"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[bus]
publishes = ["core.session.user_message"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    match validate::manifest_standalone(&m) {
        Err(ValidationError::PublishOnReservedNamespace { topic }) => {
            assert_eq!(topic, "core.session.user_message");
        }
        other => panic!("expected PublishOnReservedNamespace, got {other:?}"),
    }
}
