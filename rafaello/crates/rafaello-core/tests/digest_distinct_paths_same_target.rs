use std::fs;
use std::os::unix::fs::symlink;

use rafaello_core::digest::content_digest;

#[test]
fn distinct_logical_paths_to_same_target_both_contribute() {
    let with_link = tempfile::tempdir().unwrap();
    fs::create_dir(with_link.path().join("src")).unwrap();
    fs::write(with_link.path().join("src/lib.rs"), b"pub fn x() {}\n").unwrap();
    symlink("src", with_link.path().join("vendor_src")).unwrap();

    let without_link = tempfile::tempdir().unwrap();
    fs::create_dir(without_link.path().join("src")).unwrap();
    fs::write(without_link.path().join("src/lib.rs"), b"pub fn x() {}\n").unwrap();

    let digest_with = content_digest(with_link.path()).unwrap();
    let digest_without = content_digest(without_link.path()).unwrap();

    assert_ne!(digest_with, digest_without);
}
