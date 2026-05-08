//! `validate_with_package` rejects a manifest whose `entry`
//! points at a non-existent file (scope §M10, c11 negative).

use std::fs;

use rafaello_core::error::ManifestError;
use rafaello_core::manifest::{validate_with_package, Manifest};

const SRC: &str = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"
"#;

#[test]
fn entry_not_found_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let pkg = dir.path();
    fs::write(pkg.join("openrpc.json"), "{}").unwrap();

    let m = Manifest::parse(SRC).expect("parse");
    let err = validate_with_package(&pkg.join("rafaello.toml"), pkg, &m).expect_err("must reject");
    assert!(matches!(err, ManifestError::EntryNotFound), "got {err:?}");
}

#[test]
fn entry_is_directory_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let pkg = dir.path();
    fs::write(pkg.join("openrpc.json"), "{}").unwrap();
    fs::create_dir(pkg.join("bin")).unwrap();
    fs::create_dir(pkg.join("bin/run.sh")).unwrap();

    let m = Manifest::parse(SRC).expect("parse");
    let err = validate_with_package(&pkg.join("rafaello.toml"), pkg, &m).expect_err("must reject");
    assert!(matches!(err, ManifestError::EntryNotFile), "got {err:?}");
}
