//! c05 §B3 round-3 M-5 — `rfl install` must be invoked with EXACTLY
//! one of `<plugin>` (positional) or `--fixture <path>`; neither and
//! both trigger a clap error before `run_install` executes.

mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;

#[test]
fn rfl_install_requires_one_of_fixture_or_plugin() {
    let project = tempfile::tempdir().unwrap();
    let rfl = workspace_bin("rfl");

    let none = Command::new(&rfl)
        .current_dir(project.path())
        .args(["install"])
        .output()
        .expect("spawn rfl install");
    assert!(
        !none.status.success(),
        "neither plugin nor fixture should fail"
    );
    let stderr = String::from_utf8_lossy(&none.stderr);
    assert!(
        stderr.to_lowercase().contains("usage")
            || stderr.contains("required")
            || stderr.contains("error:"),
        "neither-args stderr lacks clap usage hint: {stderr}"
    );

    let fixture = tempfile::tempdir().unwrap();
    let both = Command::new(&rfl)
        .current_dir(project.path())
        .args(["install", "rfl-mailcat", "--fixture"])
        .arg(fixture.path())
        .output()
        .expect("spawn rfl install");
    assert!(
        !both.status.success(),
        "both plugin and fixture should fail"
    );
    let stderr = String::from_utf8_lossy(&both.stderr);
    assert!(
        stderr.contains("cannot be used with") || stderr.contains("conflicts"),
        "both-args stderr lacks clap conflict hint: {stderr}"
    );
}
