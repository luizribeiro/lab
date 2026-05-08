//! Reserved-field pre-scan rejects top-level `runtime` (scope §M2).

use rafaello_core::error::ManifestError;
use rafaello_core::manifest::Manifest;

#[test]
fn legacy_runtime_field_is_reserved() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.1.0"
entry = "main.sh"
rafaello = ">=0.1, <0.2"
runtime = "node"
"#;
    let err = Manifest::parse(src).expect_err("runtime must be reserved");
    match err {
        ManifestError::ReservedField { field, .. } => assert_eq!(field, "runtime"),
        other => panic!("expected ReservedField, got {other:?}"),
    }
}
