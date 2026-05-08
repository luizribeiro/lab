//! Negative: a `[[renderers]] kind` matching a built-in renderer
//! (scope §M7) is rejected as `ReservedRendererKind`.

use rafaello_core::error::ValidationError;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

#[test]
fn builtin_renderer_kind_rejected() {
    let src = r#"
schema = 1
name = "renderpkg"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[[renderers]]
kind = "text"
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    match validate::manifest_standalone(&m) {
        Err(ValidationError::ReservedRendererKind { kind }) => assert_eq!(kind, "text"),
        other => panic!("expected ReservedRendererKind, got {other:?}"),
    }
}
