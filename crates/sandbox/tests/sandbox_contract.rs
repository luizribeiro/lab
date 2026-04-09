mod common;

use common::{run_probe, TestDir};

use capsa_sandbox::Sandbox;

#[test]
fn read_allowlist_on_single_file_does_not_grant_siblings() {
    let temp = TestDir::new("read-contract");
    let allowed = temp.join("allowed.txt");
    let sibling = temp.join("sibling.txt");

    std::fs::write(&allowed, b"allowed").expect("failed to write allowed fixture");
    std::fs::write(&sibling, b"sibling").expect("failed to write sibling fixture");

    assert!(run_probe(
        Sandbox::builder().read_only_path(allowed.clone()),
        &["can-read", &allowed.display().to_string()]
    ));
    assert!(!run_probe(
        Sandbox::builder().read_only_path(allowed.clone()),
        &["can-read", &sibling.display().to_string()]
    ));
}

#[test]
fn stat_allowlist_on_single_file_does_not_grant_siblings() {
    let temp = TestDir::new("stat-contract");
    let allowed = temp.join("allowed.txt");
    let sibling = temp.join("sibling.txt");

    std::fs::write(&allowed, b"allowed").expect("failed to write allowed fixture");
    std::fs::write(&sibling, b"sibling").expect("failed to write sibling fixture");

    assert!(run_probe(
        Sandbox::builder().read_only_path(allowed.clone()),
        &["can-stat", &allowed.display().to_string()]
    ));
    assert!(!run_probe(
        Sandbox::builder().read_only_path(allowed.clone()),
        &["can-stat", &sibling.display().to_string()]
    ));
}

#[test]
fn write_allowlist_is_scoped_to_explicit_rw_paths() {
    let temp = TestDir::new("write-contract");
    let allowed_file = temp.join("ok.txt");
    let denied_file = temp.join("nope.txt");
    std::fs::write(&allowed_file, b"seed").expect("failed to seed allowed file");
    std::fs::write(&denied_file, b"seed").expect("failed to seed denied file");

    assert!(run_probe(
        Sandbox::builder().read_write_path(allowed_file.clone()),
        &["can-write", &allowed_file.display().to_string()]
    ));
    assert!(!run_probe(
        Sandbox::builder().read_write_path(allowed_file.clone()),
        &["can-write", &denied_file.display().to_string()]
    ));

    // Cross-platform contract: writes must stay scoped to explicit read_write_paths.
}
