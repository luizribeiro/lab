//! c27 — the stderr-warning case (allow_secrets present but not all in
//! env.pass) is also recorded in the audit row.

mod common;

use common::install_test_kit::{
    read_audit_rows, run_install, write_fixture, ALLOW_SECRETS_MANIFEST,
};

#[test]
fn audit_install_accepted_records_unused_allow_secrets() {
    let project = tempfile::tempdir().unwrap();
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path(), ALLOW_SECRETS_MANIFEST);

    let out = run_install(project.path(), fixture.path(), &[]);
    assert!(out.status.success());

    let rows = read_audit_rows(project.path());
    let row = rows
        .iter()
        .find(|(k, _)| k == "install_accepted")
        .expect("install_accepted row");
    assert_eq!(
        row.1.get("unused_allow_secrets"),
        Some(&serde_json::json!(["B"]))
    );
    assert_eq!(
        row.1.get("allow_secrets"),
        Some(&serde_json::json!(["A", "B"]))
    );
}
