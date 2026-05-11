//! c27 — benign install records exactly one `install_accepted` audit
//! row carrying the canonical id.

mod common;

use common::install_test_kit::{read_audit_rows, run_install, write_fixture, BENIGN_MANIFEST};

#[test]
fn audit_records_install_accepted_for_happy_path() {
    let project = tempfile::tempdir().unwrap();
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path(), BENIGN_MANIFEST);

    let out = run_install(project.path(), fixture.path(), &[]);
    assert!(out.status.success());

    let rows = read_audit_rows(project.path());
    let row = rows
        .iter()
        .find(|(k, _)| k == "install_accepted")
        .expect("install_accepted row");
    assert_eq!(
        row.1.get("canonical"),
        Some(&serde_json::json!("local:readfile@0.0.0"))
    );
}
