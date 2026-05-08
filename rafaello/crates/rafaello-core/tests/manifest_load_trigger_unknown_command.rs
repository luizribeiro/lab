//! Negative: a `[load] command` entry referencing a tool not in
//! `provides.tools` is rejected (scope §V1).

use rafaello_core::error::ValidationError;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

#[test]
fn unknown_command_rejected() {
    let src = r#"
schema = 1
name = "loader"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[provides]
tools = ["grep"]

[load]
command = ["nonexistent"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    match validate::manifest_standalone(&m) {
        Err(ValidationError::LoadTriggerUnknownCommand { command }) => {
            assert_eq!(command, "nonexistent");
        }
        other => panic!("expected LoadTriggerUnknownCommand, got {other:?}"),
    }
}
