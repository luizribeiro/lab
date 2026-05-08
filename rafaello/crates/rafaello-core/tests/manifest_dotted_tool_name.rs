//! Negative: a tool name containing a dot violates the M3
//! single-segment tool-name grammar `[a-z0-9_][a-z0-9_-]*` and is
//! rejected by V1.

use rafaello_core::error::ValidationError;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

#[test]
fn dotted_tool_name_rejected() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[provides]
tools = ["rust.format"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    match validate::manifest_standalone(&m) {
        Err(ValidationError::IllegalToolName { name }) => assert_eq!(name, "rust.format"),
        other => panic!("expected IllegalToolName, got {other:?}"),
    }
}
