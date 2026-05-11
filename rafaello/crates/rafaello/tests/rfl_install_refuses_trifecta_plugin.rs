//! c27 — `rfl install` against a manifest declaring all three trifecta
//! dimensions: exit non-zero, stderr contains `TrifectaRefused` plus the
//! three booleans.

mod common;

use common::install_test_kit::{run_install, write_fixture, TRIFECTA_MANIFEST};

#[test]
fn rfl_install_refuses_trifecta_plugin() {
    let project = tempfile::tempdir().unwrap();
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path(), TRIFECTA_MANIFEST);

    let out = run_install(project.path(), fixture.path(), &[]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !out.status.success(),
        "expected non-zero exit; stderr={stderr}"
    );
    assert!(
        stderr.contains("TrifectaRefused"),
        "stderr missing TrifectaRefused: {stderr}"
    );
    assert!(
        stderr.contains("reads_untrusted=true"),
        "stderr missing reads_untrusted=true: {stderr}"
    );
    assert!(
        stderr.contains("has_outbound=true"),
        "stderr missing has_outbound=true: {stderr}"
    );
    assert!(
        stderr.contains("has_workspace_write=true"),
        "stderr missing has_workspace_write=true: {stderr}"
    );
}
