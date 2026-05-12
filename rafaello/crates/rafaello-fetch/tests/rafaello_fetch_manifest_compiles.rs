//! c20 / scope §TF1: the in-tree rafaello-fetch package layout
//! satisfies `manifest::parse` + `manifest::validate_with_package`.

use std::path::PathBuf;

use rafaello_core::manifest::{self, Manifest};

fn package_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn manifest_parses_and_validates_with_package() {
    let dir = package_dir();
    let manifest_path = dir.join("rafaello.toml");
    let raw = std::fs::read_to_string(&manifest_path).expect("read manifest");
    let parsed = Manifest::parse(&raw).expect("parse manifest");
    manifest::validate_with_package(&manifest_path, &dir, &parsed)
        .expect("validate_with_package against in-tree rafaello-fetch layout");
}
