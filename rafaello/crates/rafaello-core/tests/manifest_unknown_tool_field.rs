//! Unknown sub-table inside `[provides]` is rejected at parse
//! time by serde's `deny_unknown_fields` (scope §M3, c11 negative).

use rafaello_core::manifest::Manifest;

#[test]
fn unknown_provides_subtable_rejected() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"

[provides.weird]
foo = "bar"
"#;
    let err = Manifest::parse(src).expect_err("must reject unknown provides sub-table");
    assert!(err.to_string().contains("weird"), "got: {err}");
}
