//! c05 §B3 — `rfl install nonsense` against an empty bundled tree
//! exits non-zero with a clear `BundledPluginNotFound` message. The
//! lock file and `.rafaello/plugins/` directory are NOT written.

mod common;

use std::process::Command;

use common::workspace_bin_path::workspace_bin;

#[test]
fn rfl_install_positional_unknown_plugin_errors() {
    let project = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();

    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .current_dir(project.path())
        .args(["install", "nonsense"])
        .env("RFL_BUNDLED_PLUGINS_DIR", bundled.path())
        .output()
        .expect("spawn rfl install");
    assert!(
        !out.status.success(),
        "expected non-zero exit; stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("no bundled plugin named 'nonsense'"),
        "stderr does not mention bundled-plugin-not-found: {stderr}"
    );

    assert!(
        !project.path().join("rafaello.lock").exists(),
        "lock should not be written on failed install"
    );
    let plugins = project.path().join(".rafaello").join("plugins");
    assert!(
        !plugins.exists() || plugins.read_dir().unwrap().next().is_none(),
        "PP1 plugins dir should not be populated on failure"
    );
}
