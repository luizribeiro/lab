//! `validate_with_package` rejects a manifest whose declared
//! `grant_match` file does not exist (scope §M10, c11 negative).

use std::fs;

use rafaello_core::error::ManifestError;
use rafaello_core::manifest::{validate_with_package, Manifest};

const SRC: &str = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"

[provides]
tools = ["format"]

[provides.tool.format]
grant_match = "schemas/format-grant.json"
"#;

#[test]
fn grant_match_missing_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let pkg = dir.path();
    fs::create_dir(pkg.join("bin")).unwrap();
    fs::write(pkg.join("bin/run.sh"), "#!/bin/sh\n").unwrap();
    fs::write(pkg.join("openrpc.json"), "{}").unwrap();

    let m = Manifest::parse(SRC).expect("parse");
    let err = validate_with_package(&pkg.join("rafaello.toml"), pkg, &m).expect_err("must reject");
    assert!(
        matches!(err, ManifestError::GrantMatchNotFound),
        "got {err:?}"
    );
}
