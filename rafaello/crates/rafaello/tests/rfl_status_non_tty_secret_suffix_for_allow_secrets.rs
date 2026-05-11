//! c28 — non-TTY `rfl status` renders `[SECRET: <names>]` for plugins
//! with non-empty any-bundle `GrantEnv.allow_secrets` (scope §OP6 + §A11).

mod common;

use common::install_test_kit::{run_install, run_status, write_fixture, ALLOW_SECRETS_MANIFEST};

#[test]
fn rfl_status_non_tty_secret_suffix_for_allow_secrets() {
    let project = tempfile::tempdir().unwrap();
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path(), ALLOW_SECRETS_MANIFEST);
    let out = run_install(project.path(), fixture.path(), &[]);
    assert!(
        out.status.success(),
        "install: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let out = run_status(project.path(), false);
    assert!(
        out.status.success(),
        "status: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("[SECRET: A, B]"),
        "expected '[SECRET: A, B]' suffix, got: {stdout:?}"
    );
    assert!(
        !stdout.contains("\x1b["),
        "non-TTY must not contain ANSI escapes: {stdout:?}"
    );
}
