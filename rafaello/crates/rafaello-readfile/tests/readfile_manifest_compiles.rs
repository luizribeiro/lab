//! c22 / scope §TP1 + §TP3 (pi-1 B-2, pi-2 H-5): the in-tree
//! readfile fixture parses under the live m1 schema and survives
//! package-level validation against the on-disk fixture layout
//! (`openrpc.json` sibling + resolvable `entry`).

use std::path::PathBuf;

use rafaello_core::manifest::{self, Manifest};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("rafaello-readfile")
}

#[test]
fn readfile_fixture_compiles() {
    let dir = fixture_dir();
    let manifest_path = dir.join("rafaello.toml");
    let raw = std::fs::read_to_string(&manifest_path).expect("read fixture manifest");
    let parsed = Manifest::parse(&raw).expect("parse fixture manifest");
    manifest::validate_with_package(&manifest_path, &dir, &parsed)
        .expect("validate_with_package against fixture layout");
}
