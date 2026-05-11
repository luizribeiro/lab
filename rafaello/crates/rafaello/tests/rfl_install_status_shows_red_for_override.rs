//! c28 (pi-1 M-5, moved from c40) — full pipeline: `rfl install
//! --i-know-what-im-doing` of a trifecta plugin followed by `rfl
//! status` shows the entry rendered with the red ANSI override
//! marker on TTY.

mod common;

use common::install_test_kit::{run_install, run_status, write_fixture, TRIFECTA_MANIFEST};

#[test]
fn rfl_install_status_shows_red_for_override() {
    let project = tempfile::tempdir().unwrap();
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path(), TRIFECTA_MANIFEST);
    let out = run_install(project.path(), fixture.path(), &["--i-know-what-im-doing"]);
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
        stdout.contains("\x1b[31mlocal:trifecta@0.0.0\x1b[0m"),
        "expected red ANSI around canonical id, got: {stdout:?}"
    );
}
