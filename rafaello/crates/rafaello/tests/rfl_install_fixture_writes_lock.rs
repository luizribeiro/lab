//! c27 — happy path: `rfl install --fixture <readfile>` writes a lock entry
//! with the expected digests.

mod common;

use common::install_test_kit::{run_install, write_fixture, BENIGN_MANIFEST};
use rafaello_core::digest;
use rafaello_core::manifest::Manifest;

#[test]
fn rfl_install_fixture_writes_lock() {
    let project = tempfile::tempdir().unwrap();
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path(), BENIGN_MANIFEST);

    let out = run_install(project.path(), fixture.path(), &[]);
    assert!(
        out.status.success(),
        "install failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let lock = common::install_test_kit::read_lock(project.path());
    let canonical = rafaello_core::lock::CanonicalId::parse("local:readfile@0.0.0").unwrap();
    let entry = lock.plugins.get(&canonical).expect("entry present");

    let manifest = Manifest::parse(BENIGN_MANIFEST).unwrap();
    let expected_md = digest::manifest_digest(&manifest.canonical_bytes());
    let expected_cd = digest::content_digest(&fixture.path().canonicalize().unwrap()).unwrap();
    assert_eq!(entry.manifest_digest, expected_md);
    assert_eq!(entry.digest, expected_cd);
}
