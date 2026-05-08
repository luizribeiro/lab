//! Negative: a non-built-in plugin renderer kind without a
//! `<vendor>:<kind>` prefix (Stream E §8 / pi review-4 finding 7)
//! is rejected as `UnprefixedRendererKind`.

use rafaello_core::error::ValidationError;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

#[test]
fn unprefixed_kind_rejected() {
    let src = r#"
schema = 1
name = "renderpkg"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[[renderers]]
kind = "diagram"
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    match validate::manifest_standalone(&m) {
        Err(ValidationError::UnprefixedRendererKind { kind }) => assert_eq!(kind, "diagram"),
        other => panic!("expected UnprefixedRendererKind, got {other:?}"),
    }
}

#[test]
fn dotted_unprefixed_kind_rejected() {
    let src = r#"
schema = 1
name = "renderpkg"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[[renderers]]
kind = "code.diff"
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    assert!(matches!(
        validate::manifest_standalone(&m),
        Err(ValidationError::UnprefixedRendererKind { .. })
    ));
}
