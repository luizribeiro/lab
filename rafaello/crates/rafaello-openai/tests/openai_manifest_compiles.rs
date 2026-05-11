//! Scope §OP4: the bundled openai manifest parses + passes
//! `validate_with_package` (sibling openrpc.json + entry-resolution
//! checks) against both the in-crate copy and the m5a-locks
//! fixture-tree copy.

use std::path::PathBuf;

use rafaello_core::manifest::{validate_with_package, Manifest};

fn check(package_dir: PathBuf) {
    let manifest_path = package_dir.join("rafaello.toml");
    let raw = std::fs::read_to_string(&manifest_path).expect("read manifest");
    let manifest = Manifest::parse(&raw).expect("manifest::parse");
    validate_with_package(&manifest_path, &package_dir, &manifest).expect("validate_with_package");
}

#[test]
fn openai_manifest_in_crate_validates() {
    check(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
}

#[test]
fn openai_manifest_in_fixture_tree_validates() {
    check(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("fixtures")
            .join("m5a-locks")
            .join("rafaello-openai"),
    );
}
