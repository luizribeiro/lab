//! c27 (pi-5 M-2) — install a fixture whose `allow_secrets = ["A","B"]`
//! and `env.pass = ["A"]`; assert stderr warning for `B`, lock writes
//! succeed, and audit row's `details.unused_allow_secrets == ["B"]`.

mod common;

use common::install_test_kit::{
    read_audit_rows, read_lock, run_install, write_fixture, ALLOW_SECRETS_MANIFEST,
};

#[test]
fn rfl_install_warns_on_unused_allow_secrets_entry() {
    let project = tempfile::tempdir().unwrap();
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path(), ALLOW_SECRETS_MANIFEST);

    let out = run_install(project.path(), fixture.path(), &[]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "install failed: stderr={stderr}");
    assert!(
        stderr.contains("warning: unused allow_secrets entry 'B' (no matching env.pass entry)"),
        "stderr missing unused-allow_secrets warning for 'B': {stderr}"
    );
    assert!(
        !stderr.contains("entry 'A'"),
        "stderr should not warn about 'A': {stderr}"
    );

    let lock = read_lock(project.path());
    let canonical = rafaello_core::lock::CanonicalId::parse("local:secrets@0.0.0").unwrap();
    assert!(lock.plugins.contains_key(&canonical));

    let rows = read_audit_rows(project.path());
    let install_row = rows
        .iter()
        .find(|(k, _)| k == "install_accepted")
        .expect("install_accepted row");
    let unused = install_row
        .1
        .get("unused_allow_secrets")
        .expect("unused list");
    assert_eq!(unused, &serde_json::json!(["B"]));
}
