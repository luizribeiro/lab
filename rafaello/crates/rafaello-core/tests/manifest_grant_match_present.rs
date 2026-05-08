//! `validate_with_package` resolves `grant_match` against the
//! package directory (scope §M10, c11 positive).

use std::fs;

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
fn grant_match_present_validates() {
    let dir = tempfile::tempdir().unwrap();
    let pkg = dir.path();
    fs::create_dir(pkg.join("bin")).unwrap();
    fs::create_dir(pkg.join("schemas")).unwrap();
    fs::write(pkg.join("bin/run.sh"), "#!/bin/sh\n").unwrap();
    fs::write(pkg.join("schemas/format-grant.json"), "{}").unwrap();
    fs::write(pkg.join("openrpc.json"), "{}").unwrap();
    fs::write(pkg.join("rafaello.toml"), SRC).unwrap();

    let m = Manifest::parse(SRC).expect("parse");
    validate_with_package(&pkg.join("rafaello.toml"), pkg, &m).expect("validate");
}
