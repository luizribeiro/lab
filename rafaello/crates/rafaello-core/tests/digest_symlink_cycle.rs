use std::fs;
use std::os::unix::fs::symlink;

use rafaello_core::digest::content_digest;
use rafaello_core::error::DigestError;

#[test]
fn directory_symlink_cycle_is_detected() {
    let pkg = tempfile::tempdir().unwrap();
    fs::write(pkg.path().join("ok.txt"), b"hi\n").unwrap();
    symlink(".", pkg.path().join("loop")).unwrap();

    let err = content_digest(pkg.path()).unwrap_err();
    assert!(matches!(err, DigestError::SymlinkCycle), "got {err:?}");
}
