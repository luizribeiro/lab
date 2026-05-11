//! Shared helpers for `rfl install` integration tests (scope §Tr1/§Tr4).

#![allow(dead_code)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use super::workspace_bin_path::workspace_bin;

pub const BENIGN_MANIFEST: &str = r#"
schema = 1
name = "readfile"
version = "0.0.0"
entry = "bin/x"
rafaello = ">=0.1, <0.2"

[provides]
tools = ["read-file"]

[provides.tool.read-file]
sinks = []
always_confirm = false

[capabilities.default.filesystem]
read_dirs = ["${project}"]

[capabilities.default.network]
mode = "deny"
"#;

pub const TRIFECTA_MANIFEST: &str = r#"
schema = 1
name = "trifecta"
version = "0.0.0"
entry = "bin/x"
rafaello = ">=0.1, <0.2"

[capabilities.default.filesystem]
write_dirs = ["${project}/out"]

[capabilities.default.network]
mode = "allow_all"
"#;

pub const ALLOW_SECRETS_MANIFEST: &str = r#"
schema = 1
name = "secrets"
version = "0.0.0"
entry = "bin/x"
rafaello = ">=0.1, <0.2"

[capabilities.default.env]
pass = ["A"]
allow_secrets = ["A", "B"]

[capabilities.default.network]
mode = "deny"
"#;

pub const FULL_ALLOW_SECRETS_MANIFEST: &str = r#"
schema = 1
name = "secrets"
version = "0.0.0"
entry = "bin/x"
rafaello = ">=0.1, <0.2"

[capabilities.default.env]
pass = ["LITELLM_API_KEY", "OPENAI_API_KEY", "ANTHROPIC_API_KEY"]
allow_secrets = ["LITELLM_API_KEY", "OPENAI_API_KEY", "ANTHROPIC_API_KEY"]

[capabilities.default.network]
mode = "deny"
"#;

pub fn write_fixture(dir: &Path, manifest_toml: &str) {
    fs::create_dir_all(dir.join("bin")).unwrap();
    fs::write(dir.join("rafaello.toml"), manifest_toml).unwrap();
    fs::write(dir.join("openrpc.json"), b"{}").unwrap();
    let entry = dir.join("bin").join("x");
    fs::write(&entry, b"#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&entry, fs::Permissions::from_mode(0o755)).unwrap();
}

pub fn run_install(project_root: &Path, fixture: &Path, extra: &[&str]) -> Output {
    let rfl = workspace_bin("rfl");
    let mut cmd = Command::new(rfl);
    cmd.current_dir(project_root)
        .arg("install")
        .arg("--fixture")
        .arg(fixture);
    for a in extra {
        cmd.arg(a);
    }
    cmd.output().expect("spawn rfl install")
}

pub fn read_lock(project_root: &Path) -> rafaello_core::lock::Lock {
    let raw = fs::read_to_string(project_root.join("rafaello.lock")).expect("read lock");
    rafaello_core::lock::Lock::from_toml(&raw).expect("parse lock")
}

pub fn read_audit_rows(project_root: &Path) -> Vec<(String, serde_json::Value)> {
    let db = project_root
        .join(".rafaello")
        .join("state")
        .join("session.sqlite");
    let conn = rusqlite::Connection::open(&db).expect("open audit sqlite");
    let mut stmt = conn
        .prepare("SELECT kind, payload FROM audit_events ORDER BY seq ASC")
        .expect("prep");
    let rows = stmt
        .query_map([], |row| {
            let kind: String = row.get(0)?;
            let payload: String = row.get(1)?;
            Ok((kind, payload))
        })
        .expect("query");
    rows.map(|r| {
        let (k, p) = r.unwrap();
        let v: serde_json::Value = serde_json::from_str(&p).unwrap();
        (k, v)
    })
    .collect()
}

pub fn fixture_tempdir() -> (tempfile::TempDir, PathBuf) {
    let td = tempfile::tempdir().expect("fixture tempdir");
    let p = td.path().to_path_buf();
    (td, p)
}
