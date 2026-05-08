//! Negative: `manifest.name` violating the topic-segment grammar
//! `[a-z0-9_][a-z0-9_-]*` is rejected (scope §M1 / V1).

use rafaello_core::error::ValidationError;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

#[test]
fn uppercase_manifest_name_rejected() {
    let src = r#"
schema = 1
name = "Bad-Name"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    match validate::manifest_standalone(&m) {
        Err(ValidationError::IllegalManifestName { name }) => {
            assert_eq!(name, "Bad-Name");
        }
        other => panic!("expected IllegalManifestName, got {other:?}"),
    }
}

#[test]
fn leading_hyphen_manifest_name_rejected() {
    let src = r#"
schema = 1
name = "-leading"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    assert!(matches!(
        validate::manifest_standalone(&m),
        Err(ValidationError::IllegalManifestName { .. })
    ));
}
