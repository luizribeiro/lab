//! `validate_with_package` surfaces V1's `UnknownToolTable` for
//! a `[provides.tool.<n>]` table whose `<n>` is not declared in
//! `provides.tools` (scope §V1, c11 negative).

use std::fs;

use rafaello_core::error::{ManifestError, ValidationError};
use rafaello_core::manifest::{validate_with_package, Manifest};

const SRC: &str = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"

[provides]
tools = ["grep"]

[provides.tool.unknown]
always_confirm = true
"#;

#[test]
fn orphan_tool_table_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let pkg = dir.path();
    fs::create_dir(pkg.join("bin")).unwrap();
    fs::write(pkg.join("bin/run.sh"), "#!/bin/sh\n").unwrap();
    fs::write(pkg.join("openrpc.json"), "{}").unwrap();

    let m = Manifest::parse(SRC).expect("parse");
    let err = validate_with_package(&pkg.join("rafaello.toml"), pkg, &m)
        .expect_err("must reject orphan tool table");
    match err {
        ManifestError::Validation(ValidationError::UnknownToolTable { tool }) => {
            assert_eq!(tool, "unknown");
        }
        other => panic!("expected UnknownToolTable, got {other:?}"),
    }
}
