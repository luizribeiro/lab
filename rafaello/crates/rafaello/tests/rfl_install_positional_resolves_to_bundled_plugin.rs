//! c05 §B3 — `rfl install rfl-mailcat` against
//! `RFL_BUNDLED_PLUGINS_DIR=<release-tree>` resolves the bundled
//! plugin, compiles, writes the lock entry, and materialises the
//! `.rafaello/plugins/<topic-id>/` directory.

mod common;

use std::process::Command;

use common::install_test_kit::write_bundled_plugin;
use common::workspace_bin_path::workspace_bin;
use rafaello_core::lock::{CanonicalId, Lock};
use rafaello_core::topic_id;

#[test]
fn rfl_install_positional_resolves_to_bundled_plugin() {
    let project = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    write_bundled_plugin(bundled.path(), "rfl-mailcat", "rfl-mailcat");

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
    let canonical = CanonicalId::parse("local:rfl-mailcat@0.0.0").unwrap();
    let entry = lock.plugins.get(&canonical).expect("plugin entry");
    assert_eq!(entry.entry.as_str(), "bin/rfl-mailcat");

    let topic = topic_id::derive("local:rfl-mailcat@0.0.0");
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
