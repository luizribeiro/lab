//! Reserved-field pre-scan rejects top-level `helper_for` (scope §M2).

use rafaello_core::error::ManifestError;
use rafaello_core::manifest::Manifest;

#[test]
fn helper_for_field_is_reserved() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.1.0"
entry = "main.sh"
rafaello = ">=0.1, <0.2"
helper_for = "some-other-plugin"
"#;
    let err = Manifest::parse(src).expect_err("helper_for must be reserved");
    match err {
        ManifestError::ReservedField { field, .. } => assert_eq!(field, "helper_for"),
        other => panic!("expected ReservedField, got {other:?}"),
    }
}
