//! c27 — install a fixture whose `allow_secrets` is non-empty; the
//! `install_accepted` audit row carries `details.allow_secrets: [...]`.

mod common;

use common::install_test_kit::{
    read_audit_rows, run_install, write_fixture, FULL_ALLOW_SECRETS_MANIFEST,
};

#[test]
fn audit_install_accepted_records_allow_secrets_list() {
    let project = tempfile::tempdir().unwrap();
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path(), FULL_ALLOW_SECRETS_MANIFEST);

    let out = run_install(project.path(), fixture.path(), &[]);
    assert!(
        out.status.success(),
        "install failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let rows = read_audit_rows(project.path());
    let row = rows
        .iter()
        .find(|(k, _)| k == "install_accepted")
        .expect("install_accepted row");
    let list = row.1.get("allow_secrets").expect("allow_secrets list");
    assert_eq!(
        list,
        &serde_json::json!(["ANTHROPIC_API_KEY", "LITELLM_API_KEY", "OPENAI_API_KEY"])
    );
}
