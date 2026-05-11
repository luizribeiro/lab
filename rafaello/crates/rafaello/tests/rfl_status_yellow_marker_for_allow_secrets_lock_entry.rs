//! c28 — `rfl status` renders a yellow ANSI `explicit secret: <names>`
//! suffix on TTY when an installed plugin's any-bundle
//! `GrantEnv.allow_secrets` is non-empty (scope §OP6 + §A11).

mod common;

use common::install_test_kit::{run_install, run_status, write_fixture, ALLOW_SECRETS_MANIFEST};

#[test]
fn rfl_status_yellow_marker_for_allow_secrets_lock_entry() {
    let project = tempfile::tempdir().unwrap();
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path(), ALLOW_SECRETS_MANIFEST);
    let out = run_install(project.path(), fixture.path(), &[]);
    assert!(
        out.status.success(),
        "install: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run_status(project.path(), true);
    assert!(
        out.status.success(),
        "status: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("\x1b[33mexplicit secret: A, B\x1b[0m"),
        "expected yellow ANSI 'explicit secret: A, B' suffix, got: {stdout:?}"
    );
}
