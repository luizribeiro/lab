//! c05 §B3 round-4 B-1 — after `rfl install rfl-mailcat`, calling
//! `rafaello_core::compile::resolve_entry(&plugin_dir,
//! &manifest.entry)` against the materialised PP1 dir returns
//! `Ok(<canonical-path>)` rooted inside
//! `.rafaello/plugins/<topic-id>/`. This proves the PP1 copy
//! preserves the entry file as a real executable file (per pi-1 M-1
//! fold: `resolve_entry` made public).

mod common;

use std::process::Command;

use common::install_test_kit::write_bundled_plugin;
use common::workspace_bin_path::workspace_bin;
use rafaello_core::compile;
use rafaello_core::manifest::Manifest;
use rafaello_core::topic_id;

#[test]
fn rfl_install_resolves_entry_against_canonicalised_package_dir() {
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
        "rfl install failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let topic = topic_id::derive("local:rfl-mailcat@0.0.0");
    let plugin_dir = project
        .path()
        .join(".rafaello")
        .join("plugins")
        .join(&topic);

    let manifest_raw = std::fs::read_to_string(plugin_dir.join("rafaello.toml")).unwrap();
    let manifest = Manifest::parse(&manifest_raw).unwrap();

    let resolved = compile::resolve_entry(&plugin_dir, manifest.entry.as_str())
        .expect("resolve_entry must succeed against installed PP1 dir");
    let canon_root = std::fs::canonicalize(&plugin_dir).unwrap();
    assert!(
        resolved.starts_with(&canon_root),
        "resolved entry {resolved:?} must be inside {canon_root:?}"
    );
    assert!(resolved.is_file(), "resolved entry must be a real file");
}
