//! c28 — `rfl status` prints `[OVERRIDE]` prefix and no ANSI codes
//! when stdout is not a TTY (scope §Tr3).

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
fn rfl_status_prints_override_prefix_for_non_tty() {
    let project = tempfile::tempdir().unwrap();
    fs::write(project.path().join("rafaello.lock"), LOCK_WITH_OVERRIDE).unwrap();

    let out = run_status(project.path(), false);
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(
        stdout.starts_with("[OVERRIDE] local:demo@0.0.0"),
        "expected [OVERRIDE] prefix, got: {stdout:?}"
    );
    assert!(
        !stdout.contains("\x1b["),
        "non-TTY output must not contain ANSI escapes: {stdout:?}"
    );
}
