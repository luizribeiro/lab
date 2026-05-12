//! c05 §B3 — `rfl install rfl-mailcat` against
//! `RFL_BUNDLED_PLUGINS_DIR=<release-tree>` resolves the bundled
//! plugin, compiles, writes the lock entry, and materialises the
//! `.rafaello/plugins/<topic-id>/` directory.
//!
//! c06 §B2 amend — the synthetic bundled tree is now constructed
//! from the in-tree `rafaello-mailcat` crate files (`rafaello.toml`,
//! `openrpc.json`, `bin/rfl-mailcat`, `schemas/send-mail-grant.json`),
//! pinning the happy-path positional resolver against the canonical
//! in-tree manifest shape that Phase F2's `postInstall` will copy
//! into `$out/share/rafaello/plugins/rfl-mailcat/`.

mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::lock::{CanonicalId, Lock};
use rafaello_core::topic_id;

fn mailcat_crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("rafaello-mailcat")
}

fn copy_file(src: &Path, dst: &Path, executable: bool) {
    fs::create_dir_all(dst.parent().unwrap()).unwrap();
    fs::copy(src, dst).unwrap_or_else(|e| panic!("copy {src:?} -> {dst:?}: {e}"));
    if executable {
        fs::set_permissions(dst, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn write_bundled_mailcat_from_intree(root: &Path) {
    let src = mailcat_crate_dir();
    let dst = root.join("rfl-mailcat");
    copy_file(
        &src.join("rafaello.toml"),
        &dst.join("rafaello.toml"),
        false,
    );
    copy_file(&src.join("openrpc.json"), &dst.join("openrpc.json"), false);
    copy_file(
        &src.join("bin").join("rfl-mailcat"),
        &dst.join("bin").join("rfl-mailcat"),
        true,
    );
    copy_file(
        &src.join("schemas").join("send-mail-grant.json"),
        &dst.join("schemas").join("send-mail-grant.json"),
        false,
    );
}

#[test]
fn rfl_install_positional_resolves_to_bundled_plugin() {
    let project = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    write_bundled_mailcat_from_intree(bundled.path());

    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .current_dir(project.path())
        .args(["install", "rfl-mailcat"])
        .env("RFL_BUNDLED_PLUGINS_DIR", bundled.path())
        .output()
        .expect("spawn rfl install");
    assert!(
        out.status.success(),
        "rfl install rfl-mailcat failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let lock_raw =
        std::fs::read_to_string(project.path().join("rafaello.lock")).expect("read lock");
    let lock = Lock::from_toml(&lock_raw).expect("parse lock");
    let canonical = CanonicalId::parse("local:mailcat@0.0.0").unwrap();
    let entry = lock.plugins.get(&canonical).expect("plugin entry");
    assert_eq!(entry.entry.as_str(), "bin/rfl-mailcat");

    let topic = topic_id::derive("local:mailcat@0.0.0");
    let pp1 = project
        .path()
        .join(".rafaello")
        .join("plugins")
        .join(&topic);
    assert!(pp1.join("rafaello.toml").is_file(), "PP1 manifest missing");
    assert!(
        pp1.join("bin").join("rfl-mailcat").is_file(),
        "PP1 entry missing"
    );
}
