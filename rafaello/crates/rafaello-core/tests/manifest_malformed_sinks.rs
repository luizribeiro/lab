//! Negative: an uppercase sink class string violates the M3
//! sink-class grammar (known set + `[a-z0-9_]+` custom). Uppercase
//! grammar half is V1's responsibility (scope §V1, c10);
//! `manifest_malformed_sinks_type.rs` covers the type-mismatch
//! half at parse time (c05).

use rafaello_core::error::ValidationError;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

#[test]
fn uppercase_sink_class_rejected() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"

[provides]
tools = ["grep"]

[provides.tool.grep]
sinks = ["Network"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    match validate::manifest_standalone(&m) {
        Err(ValidationError::IllegalSinkClass { class }) => assert_eq!(class, "Network"),
        other => panic!("expected IllegalSinkClass, got {other:?}"),
    }
}
