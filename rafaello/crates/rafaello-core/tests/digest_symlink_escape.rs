use std::fs;
use std::os::unix::fs::symlink;

use rafaello_core::digest::content_digest;
use rafaello_core::error::DigestError;

#[test]
fn symlink_target_outside_package_dir_is_refused() {
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.txt"), b"oops\n").unwrap();

    let pkg = tempfile::tempdir().unwrap();
    fs::write(pkg.path().join("ok.txt"), b"hi\n").unwrap();
    symlink(
        outside.path().join("secret.txt"),
        pkg.path().join("escape"),
    )
    .unwrap();

    let err = content_digest(pkg.path()).unwrap_err();
    assert!(matches!(err, DigestError::SymlinkEscape), "got {err:?}");
}
