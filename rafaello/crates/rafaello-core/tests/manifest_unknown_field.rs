//! Unknown field rejection at top-level and inside a nested
//! `[provides.tool.<n>]` table (scope §M1, c11 negative).

use rafaello_core::manifest::Manifest;

#[test]
fn top_level_unknown_field_rejected() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"
mystery = "value"
"#;
    let err = Manifest::parse(src).expect_err("must reject unknown top-level field");
    assert!(err.to_string().contains("mystery"), "got: {err}");
}

#[test]
fn nested_tool_meta_unknown_field_rejected() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"

[provides]
tools = ["grep"]

[provides.tool.grep]
unknown_field = "value"
"#;
    let err = Manifest::parse(src).expect_err("must reject unknown tool-meta field");
    assert!(err.to_string().contains("unknown_field"), "got: {err}");
}
