//! c05 §B3 round-4 B-2 — `rfl install rfl-mailcat --project-root
//! <tmpdir>` invoked from a different cwd writes the lock + PP1 dir
//! under `<tmpdir>`, NOT under the invoking cwd.

mod common;

use std::process::Command;

use common::install_test_kit::write_bundled_plugin;
use common::workspace_bin_path::workspace_bin;
use rafaello_core::topic_id;

#[test]
fn rfl_install_project_root_flag() {
    let project = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    write_bundled_plugin(bundled.path(), "rfl-mailcat", "rfl-mailcat");

    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .current_dir(elsewhere.path())
        .args(["install", "rfl-mailcat", "--project-root"])
        .arg(project.path())
        .env("RFL_BUNDLED_PLUGINS_DIR", bundled.path())
        .output()
        .expect("spawn rfl install");
    assert!(
        out.status.success(),
        "rfl install failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(
        project.path().join("rafaello.lock").is_file(),
        "lock not written under --project-root"
    );
    let topic = topic_id::derive("local:rfl-mailcat@0.0.0");
    assert!(
        project
            .path()
            .join(".rafaello")
            .join("plugins")
            .join(&topic)
            .join("rafaello.toml")
            .is_file(),
        "PP1 dir not written under --project-root"
    );

    assert!(
        !elsewhere.path().join("rafaello.lock").exists(),
        "lock leaked into invoking cwd"
    );
    assert!(
        !elsewhere.path().join(".rafaello").exists(),
        ".rafaello/ leaked into invoking cwd"
    );
}
