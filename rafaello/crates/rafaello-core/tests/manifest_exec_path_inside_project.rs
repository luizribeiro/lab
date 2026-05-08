//! `validate_with_package` rejects any `exec_paths` /
//! `exec_dirs` entry whose template anchors under `${project}`
//! (scope §V1 + security RFC §6.9, c11 negative).
//!
//! The full resolve-against-cwd-project check is V3 territory
//! (c27); the manifest-side check is intentionally syntactic.

use std::fs;

use rafaello_core::error::ManifestError;
use rafaello_core::manifest::{validate_with_package, Manifest};

fn pkg_with_basics(dir: &std::path::Path) {
    fs::create_dir(dir.join("bin")).unwrap();
    fs::write(dir.join("bin/run.sh"), "#!/bin/sh\n").unwrap();
    fs::write(dir.join("openrpc.json"), "{}").unwrap();
}

#[test]
fn exec_paths_under_project_rejected() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"

[capabilities.default.filesystem]
exec_paths = ["${project}/scripts/runner"]
"#;
    let dir = tempfile::tempdir().unwrap();
    pkg_with_basics(dir.path());
    let m = Manifest::parse(src).expect("parse");
    let err = validate_with_package(&dir.path().join("rafaello.toml"), dir.path(), &m)
        .expect_err("must reject");
    assert!(
        matches!(err, ManifestError::ExecPathInsideProject),
        "got {err:?}"
    );
}

#[test]
fn exec_dirs_under_project_rejected() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"

[capabilities.default.filesystem]
exec_dirs = ["${project}/tools"]
"#;
    let dir = tempfile::tempdir().unwrap();
    pkg_with_basics(dir.path());
    let m = Manifest::parse(src).expect("parse");
    let err = validate_with_package(&dir.path().join("rafaello.toml"), dir.path(), &m)
        .expect_err("must reject");
    assert!(
        matches!(err, ManifestError::ExecPathInsideProject),
        "got {err:?}"
    );
}

#[test]
fn exec_path_outside_project_allowed() {
    let src = r#"
schema = 1
name = "rust-tools"
version = "0.3.1"
entry = "bin/run.sh"
rafaello = ">=0.1, <0.2"

[capabilities.default.filesystem]
exec_paths = ["/usr/bin/rustc", "${home}/.cargo/bin/cargo"]
"#;
    let dir = tempfile::tempdir().unwrap();
    pkg_with_basics(dir.path());
    let m = Manifest::parse(src).expect("parse");
    validate_with_package(&dir.path().join("rafaello.toml"), dir.path(), &m)
        .expect("absolute + non-project placeholder paths must validate");
}
