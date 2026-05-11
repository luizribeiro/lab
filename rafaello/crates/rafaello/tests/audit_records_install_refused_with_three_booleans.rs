//! c27 — refused install records `install_refused` with the three
//! trifecta booleans in the payload.

mod common;

use common::install_test_kit::{read_audit_rows, run_install, write_fixture, TRIFECTA_MANIFEST};

#[test]
fn audit_records_install_refused_with_three_booleans() {
    let project = tempfile::tempdir().unwrap();
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path(), TRIFECTA_MANIFEST);

    let out = run_install(project.path(), fixture.path(), &[]);
    assert!(!out.status.success());

    let rows = read_audit_rows(project.path());
    let row = rows
        .iter()
        .find(|(k, _)| k == "install_refused")
        .expect("install_refused row");
    assert_eq!(row.1.get("reads_untrusted"), Some(&serde_json::json!(true)));
    assert_eq!(row.1.get("has_outbound"), Some(&serde_json::json!(true)));
    assert_eq!(
        row.1.get("has_workspace_write"),
        Some(&serde_json::json!(true))
    );
}
