//! c28 — `rfl status` renders the canonical id wrapped in red ANSI
//! when `flags.i_know_what_im_doing == true` on a TTY (scope §Tr3,
//! security RFC §7.1 "loud surfacing").

mod common;

use std::fs;

use common::install_test_kit::run_status;

const LOCK_WITH_OVERRIDE: &str = r#"
[plugin."local:demo@0.0.0"]
entry = "bin/x"
digest = "sha256:0000000000000000000000000000000000000000000000000000000000000000"
manifest_digest = "sha256:1111111111111111111111111111111111111111111111111111111111111111"
granted_at = "2026-05-11T00:00:00Z"

[plugin."local:demo@0.0.0".flags]
i_know_what_im_doing = true
"#;

#[test]
fn rfl_status_prints_red_for_override_flag() {
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("rafaello.lock"), LOCK_WITH_OVERRIDE).unwrap();

    let out = run_status(project.path(), true);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.contains("\x1b[31mlocal:demo@0.0.0\x1b[0m"),
        "expected red ANSI around canonical id, got: {stdout:?}"
    );
    assert!(
        !stdout.contains("[OVERRIDE]"),
        "TTY mode must not print [OVERRIDE] prefix"
    );
}
