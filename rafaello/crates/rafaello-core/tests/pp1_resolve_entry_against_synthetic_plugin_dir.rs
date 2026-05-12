//! c17 §F2 + PP1 — `compile::resolve_entry` against a synthetic
//! plugin dir matching the F2 layout
//! (`<dir>/rafaello.toml`, `<dir>/openrpc.json`,
//! `<dir>/bin/<plugin-bin>` as a real file) returns the canonical
//! entry path inside the plugin dir.
//!
//! This is the pure-Rust replacement for the dropped round-2
//! `nix_build_layout.rs` Cargo→Nix recursive integration test.

use std::fs;
use std::io::Write;

use rafaello_core::compile::resolve_entry;

fn write_file(path: &std::path::Path, body: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    let mut f = fs::File::create(path).unwrap();
    f.write_all(body.as_bytes()).unwrap();
}

#[test]
fn resolve_entry_returns_canonical_real_file_inside_plugin_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("rfl-mailcat");
    fs::create_dir_all(&dir).unwrap();

    write_file(&dir.join("rafaello.toml"), "name = \"rfl-mailcat\"\n");
    write_file(&dir.join("openrpc.json"), "{}\n");

    let bin_rel = "bin/rfl-mailcat";
    let bin_abs = dir.join(bin_rel);
    write_file(&bin_abs, "#!/bin/sh\nexit 0\n");

    let canon = resolve_entry(&dir, bin_rel).expect("resolve_entry ok");

    let dir_canon = fs::canonicalize(&dir).unwrap();
    assert!(
        canon.starts_with(&dir_canon),
        "canonical entry {:?} should live inside plugin dir {:?}",
        canon,
        dir_canon,
    );
    assert_eq!(canon, fs::canonicalize(&bin_abs).unwrap());
    assert!(canon.is_file());
    let lmeta = fs::symlink_metadata(&canon).unwrap();
    assert!(
        !lmeta.file_type().is_symlink(),
        "PP1 containment requires the plugin binary to be a real file, not a symlink",
    );
}
