//! `validate_with_package` rejects a tool plugin without an
//! `openrpc.json` sibling (scope §M10, c11 negative).

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
tools = ["grep"]
"#;

#[test]
fn missing_openrpc_sibling_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let pkg = dir.path();
    fs::create_dir(pkg.join("bin")).unwrap();
    fs::write(pkg.join("bin/run.sh"), "#!/bin/sh\n").unwrap();
    fs::write(pkg.join("rafaello.toml"), SRC).unwrap();

    let m = Manifest::parse(SRC).expect("parse");
    let err = validate_with_package(&pkg.join("rafaello.toml"), pkg, &m)
        .expect_err("must reject missing openrpc.json");
    assert!(matches!(err, ManifestError::MissingOpenRpc), "got {err:?}");
}
