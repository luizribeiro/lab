//! c30 / scope §TP1 + pi-1 B-9: the in-tree mailcat package layout
//! satisfies `manifest::validate_with_package` — `openrpc.json`
//! sibling present, `entry` (`bin/rfl-mailcat`) resolves inside the
//! package, and `grant_match` (`schemas/send-mail-grant.json`)
//! resolves inside the package.

use std::path::PathBuf;

use rafaello_core::manifest::{self, Manifest};

fn package_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn validate_with_package_succeeds() {
    let dir = package_dir();
    let manifest_path = dir.join("rafaello.toml");
    let raw = std::fs::read_to_string(&manifest_path).expect("read manifest");
    let parsed = Manifest::parse(&raw).expect("parse manifest");
    manifest::validate_with_package(&manifest_path, &dir, &parsed)
        .expect("validate_with_package against in-tree mailcat layout");
}
