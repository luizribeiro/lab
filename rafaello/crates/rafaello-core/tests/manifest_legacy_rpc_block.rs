//! Reserved-field pre-scan rejects an `[rpc]` table (scope §M2).

use rafaello_core::error::ManifestError;
use rafaello_core::manifest::Manifest;

#[test]
fn legacy_rpc_block_is_reserved() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.1.0"
entry = "main.sh"
rafaello = ">=0.1, <0.2"

[rpc]
methods = ["foo.bar"]
"#;
    let err = Manifest::parse(src).expect_err("[rpc] must be reserved");
    match err {
        ManifestError::ReservedField { field, .. } => assert_eq!(field, "rpc"),
        other => panic!("expected ReservedField, got {other:?}"),
    }
}
