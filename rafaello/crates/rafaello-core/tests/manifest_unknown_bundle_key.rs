//! Negative: a `[capabilities.<bundle>]` key that is neither
//! `default` nor a name in `provides.tools` is rejected as
//! `UnknownBundleKey` (M5 / V1 / pi review-1 finding 3).

use rafaello_core::error::ValidationError;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

#[test]
fn unknown_bundle_key_rejected() {
    let src = r#"
schema = 1
name = "boundary"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[provides]
tools = ["grep"]

[capabilities.typo.filesystem]
read_paths = ["/etc/hosts"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    match validate::manifest_standalone(&m) {
        Err(ValidationError::UnknownBundleKey { bundle }) => assert_eq!(bundle, "typo"),
        other => panic!("expected UnknownBundleKey, got {other:?}"),
    }
}
