//! `validate_with_package` rejects an `entry` whose canonical
//! target leaves the package via a symlink (scope §M10, c11
//! negative).

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::symlink;

use rafaello_core::error::ManifestError;
use rafaello_core::manifest::{validate_with_package, Manifest};

const SRC: &str = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "run.sh"
rafaello = ">=0.1, <0.2"
"#;

#[test]
fn entry_symlink_escape_rejected() {
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("evil.sh"), "#!/bin/sh\n").unwrap();

    let dir = tempfile::tempdir().unwrap();
    let pkg = dir.path();
    fs::write(pkg.join("openrpc.json"), "{}").unwrap();
    symlink(outside.path().join("evil.sh"), pkg.join("run.sh")).unwrap();

    let m = Manifest::parse(SRC).expect("parse");
    let err = validate_with_package(&pkg.join("rafaello.toml"), pkg, &m).expect_err("must reject");
    assert!(matches!(err, ManifestError::EntryEscape), "got {err:?}");
}
