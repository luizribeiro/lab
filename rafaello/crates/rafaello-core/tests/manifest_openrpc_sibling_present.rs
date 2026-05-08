//! `validate_with_package` succeeds when the openrpc.json sibling
//! exists alongside the manifest (scope §M10, c11 positive).

use std::fs;

use rafaello_core::manifest::{validate_with_package, Manifest};

const SRC: &str = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"
"#;

#[test]
fn openrpc_sibling_present_validates() {
    let dir = tempfile::tempdir().expect("tempdir");
    let pkg = dir.path();
    fs::create_dir(pkg.join("bin")).unwrap();
    fs::write(pkg.join("bin/run.sh"), "#!/bin/sh\n").unwrap();
    fs::write(pkg.join("openrpc.json"), "{\"methods\":[]}").unwrap();
    fs::write(pkg.join("rafaello.toml"), SRC).unwrap();

    let m = Manifest::parse(SRC).expect("parse");
    validate_with_package(&pkg.join("rafaello.toml"), pkg, &m).expect("validate");
}
