//! c05 — `rfl install --help` lists the positional `plugin` argument,
//! the `--fixture <path>` option, and the `--project-root <path>`
//! option (scope §B1 clap cutover).

mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;

#[test]
fn rfl_install_help_lists_positional_and_fixture() {
    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .args(["install", "--help"])
        .output()
        .expect("spawn rfl install --help");
    assert!(
        out.status.success(),
        "rfl install --help failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("[PLUGIN]") || stdout.contains("<PLUGIN>"),
        "help missing positional plugin argument: {stdout}"
    );
    assert!(
        stdout.contains("--fixture"),
        "help missing --fixture: {stdout}"
    );
    assert!(
        stdout.contains("--project-root"),
        "help missing --project-root: {stdout}"
    );
}
