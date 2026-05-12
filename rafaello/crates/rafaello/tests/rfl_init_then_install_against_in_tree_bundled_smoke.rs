//! c07 §A4 (pi-2 B-1 fold — moved from c04) — `rfl init --yes`
//! against a bundled tempdir constructed from the **in-tree**
//! `rafaello/crates/rafaello-openai/` source (the manifest +
//! `openrpc.json` promoted in c06, plus a stub entry script)
//! parses post-init under PP1 and its `manifest_digest` in the
//! lock matches `digest::manifest_digest` over the canonical
//! bytes of the copied manifest.

mod common;

use std::fs;
use std::process::Command;

use common::install_test_kit::copy_in_tree_to_bundled_dir;
use common::workspace_bin_path::workspace_bin;
use rafaello_core::digest;
use rafaello_core::lock::Lock;
use rafaello_core::manifest::Manifest;
use rafaello_core::topic_id;

const OPENAI_CANONICAL: &str = "builtin:openai@0.0.0";

#[test]
fn rfl_init_then_install_against_in_tree_bundled_smoke() {
    let project = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    copy_in_tree_to_bundled_dir(bundled.path(), "openai", "rafaello-openai");

    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .args(["init", "--yes", "--project-root"])
        .arg(project.path())
        .env("RFL_BUNDLED_PLUGINS_DIR", bundled.path())
        .env("RFL_BUNDLED_BIN_OPENAI", workspace_bin("rfl-openai-stub"))
        .output()
        .expect("spawn rfl init");
    assert!(
        out.status.success(),
        "rfl init failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let topic = topic_id::derive(OPENAI_CANONICAL);
    let materialised_manifest = project
        .path()
        .join(".rafaello")
        .join("plugins")
        .join(&topic)
        .join("rafaello.toml");
    let manifest_raw = fs::read_to_string(&materialised_manifest)
        .unwrap_or_else(|e| panic!("read {materialised_manifest:?}: {e}"));
    let manifest = Manifest::parse(&manifest_raw).expect("materialised manifest must parse");

    let lock_raw = fs::read_to_string(project.path().join("rafaello.lock")).unwrap();
    let lock = Lock::from_toml(&lock_raw).unwrap();
    let entry = lock
        .plugins
        .iter()
        .find(|(k, _)| k.to_string() == OPENAI_CANONICAL)
        .map(|(_, v)| v)
        .expect("openai plugin entry present in lock");

    let expected = digest::manifest_digest(&manifest.canonical_bytes());
    assert_eq!(
        entry.manifest_digest, expected,
        "lock manifest_digest must match digest over in-tree-copied manifest's canonical bytes"
    );
}
