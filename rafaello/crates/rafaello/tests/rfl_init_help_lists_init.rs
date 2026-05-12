//! c01 — `rfl init --help` exits 0 and lists the three flags
//! `--yes`, `--force`, `--project-root` (scope §A1).

mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;

#[test]
fn rfl_init_help_lists_init() {
    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .arg("init")
        .arg("--help")
        .output()
        .expect("spawn rfl init --help");
    assert!(
        out.status.success(),
        "rfl init --help failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("--yes"), "missing --yes in help: {stdout}");
    assert!(
        stdout.contains("--force"),
        "missing --force in help: {stdout}"
    );
    assert!(
        stdout.contains("--project-root"),
        "missing --project-root in help: {stdout}"
    );
}
