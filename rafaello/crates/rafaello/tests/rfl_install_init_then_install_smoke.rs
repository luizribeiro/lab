//! c07 §B3 — `rfl init --yes` then `rfl install rfl-mailcat`
//! composed against the same `--project-root` lands both the
//! `builtin:openai@0.0.0` (from init) and `local:mailcat@0.0.0`
//! (from install) entries in the lock and materialises both PP1
//! package directories.

mod common;

use std::process::Command;

use common::install_test_kit::copy_in_tree_to_bundled_dir;
use common::workspace_bin_path::workspace_bin;
use rafaello_core::lock::{CanonicalId, Lock};
use rafaello_core::topic_id;

const OPENAI_CANONICAL: &str = "builtin:openai@0.0.0";
const MAILCAT_CANONICAL: &str = "local:mailcat@0.0.0";

#[test]
fn rfl_install_init_then_install_smoke() {
    let project = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    copy_in_tree_to_bundled_dir(bundled.path(), "openai", "rafaello-openai");
    copy_in_tree_to_bundled_dir(bundled.path(), "rfl-mailcat", "rafaello-mailcat");

    let rfl = workspace_bin("rfl");
    let init_out = Command::new(&rfl)
        .args(["init", "--yes", "--project-root"])
        .arg(project.path())
        .env("RFL_BUNDLED_PLUGINS_DIR", bundled.path())
        .output()
        .expect("spawn rfl init");
    assert!(
        init_out.status.success(),
        "rfl init failed: stderr={}",
        String::from_utf8_lossy(&init_out.stderr)
    );

    let install_out = Command::new(&rfl)
        .args(["install", "rfl-mailcat", "--project-root"])
        .arg(project.path())
        .env("RFL_BUNDLED_PLUGINS_DIR", bundled.path())
        .output()
        .expect("spawn rfl install");
    assert!(
        install_out.status.success(),
        "rfl install rfl-mailcat failed: stderr={}",
        String::from_utf8_lossy(&install_out.stderr)
    );

    let lock_raw = std::fs::read_to_string(project.path().join("rafaello.lock")).unwrap();
    let lock = Lock::from_toml(&lock_raw).unwrap();
    let openai = CanonicalId::parse(OPENAI_CANONICAL).unwrap();
    let mailcat = CanonicalId::parse(MAILCAT_CANONICAL).unwrap();
    assert!(
        lock.plugins.contains_key(&openai),
        "lock missing {OPENAI_CANONICAL}"
    );
    assert!(
        lock.plugins.contains_key(&mailcat),
        "lock missing {MAILCAT_CANONICAL}"
    );

    let plugins_root = project.path().join(".rafaello").join("plugins");
    let openai_pp1 = plugins_root.join(topic_id::derive(OPENAI_CANONICAL));
    let mailcat_pp1 = plugins_root.join(topic_id::derive(MAILCAT_CANONICAL));
    assert!(
        openai_pp1.join("rafaello.toml").is_file(),
        "openai PP1 manifest missing at {openai_pp1:?}"
    );
    assert!(
        mailcat_pp1.join("rafaello.toml").is_file(),
        "mailcat PP1 manifest missing at {mailcat_pp1:?}"
    );
}
