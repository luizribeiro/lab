//! c27 — `--i-know-what-im-doing` install records a `trifecta_overridden`
//! audit row instead of `install_accepted`.

mod common;

use common::install_test_kit::{read_audit_rows, run_install, write_fixture, TRIFECTA_MANIFEST};

#[test]
fn audit_records_trifecta_override_at_install() {
    let project = tempfile::tempdir().unwrap();
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path(), TRIFECTA_MANIFEST);

    let out = run_install(project.path(), fixture.path(), &["--i-know-what-im-doing"]);
    assert!(out.status.success());

    let rows = read_audit_rows(project.path());
    assert!(rows.iter().any(|(k, _)| k == "trifecta_overridden"));
    assert!(!rows.iter().any(|(k, _)| k == "install_accepted"));
}
