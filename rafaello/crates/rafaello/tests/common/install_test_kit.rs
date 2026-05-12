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

pub fn run_status(project_root: &Path, force_tty: bool) -> Output {
    let rfl = workspace_bin("rfl");
    let mut cmd = Command::new(rfl);
    cmd.current_dir(project_root).arg("status");
    if force_tty {
        cmd.env("RFL_STATUS_FORCE_TTY", "1");
    } else {
        cmd.env("RFL_STATUS_FORCE_NO_TTY", "1");
    }
    cmd.output().expect("spawn rfl status")
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

/// Copy an in-tree `rafaello-*` crate's bundled-plugin files
/// (`rafaello.toml`, `openrpc.json`, `schemas/*` if present) into
/// `<root>/<bundled_name>/`, writing a stub executable at the
/// manifest's `entry` path. Returns the populated plugin dir.
pub fn copy_in_tree_to_bundled_dir(
    root: &Path,
    bundled_name: &str,
    crate_dir_name: &str,
) -> PathBuf {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(crate_dir_name);
    let dst = root.join(bundled_name);
    fs::create_dir_all(&dst).unwrap();
    fs::copy(src.join("rafaello.toml"), dst.join("rafaello.toml"))
        .unwrap_or_else(|e| panic!("copy rafaello.toml from {src:?}: {e}"));
    fs::copy(src.join("openrpc.json"), dst.join("openrpc.json"))
        .unwrap_or_else(|e| panic!("copy openrpc.json from {src:?}: {e}"));

    let manifest_raw = fs::read_to_string(dst.join("rafaello.toml")).unwrap();
    let manifest = rafaello_core::manifest::Manifest::parse(&manifest_raw)
        .expect("in-tree manifest must parse");
    let entry_dst = dst.join(manifest.entry.as_str());
    fs::create_dir_all(entry_dst.parent().unwrap()).unwrap();
    fs::write(&entry_dst, b"#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&entry_dst, fs::Permissions::from_mode(0o755)).unwrap();

    let schemas_src = src.join("schemas");
    if schemas_src.is_dir() {
        let schemas_dst = dst.join("schemas");
        fs::create_dir_all(&schemas_dst).unwrap();
        for entry in fs::read_dir(&schemas_src).unwrap() {
            let e = entry.unwrap();
            fs::copy(e.path(), schemas_dst.join(e.file_name())).unwrap();
        }
    }
    dst
}

/// Write a synthetic bundled-plugin tree at `<root>/<plugin_name>/`
/// suitable for `RFL_BUNDLED_PLUGINS_DIR=<root>` lookup.
pub fn write_bundled_plugin(root: &Path, plugin_name: &str, manifest_name: &str) {
    let plugin_dir = root.join(plugin_name);
    fs::create_dir_all(plugin_dir.join("bin")).unwrap();
    let entry_rel = format!("bin/{plugin_name}");
    let manifest = format!(
        r#"schema = 1
name = "{manifest_name}"
version = "0.0.0"
entry = "{entry_rel}"
rafaello = ">=0.1, <0.2"

[provides]
tools = ["{plugin_name}-tool"]

[provides.tool.{plugin_name}-tool]
sinks = []
always_confirm = false

[capabilities.default.filesystem]
read_dirs = ["${{project}}"]

[capabilities.default.network]
mode = "deny"
"#
    );
    fs::write(plugin_dir.join("rafaello.toml"), manifest).unwrap();
    fs::write(plugin_dir.join("openrpc.json"), b"{}").unwrap();
    let bin = plugin_dir.join(&entry_rel);
    fs::write(&bin, b"#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
}
