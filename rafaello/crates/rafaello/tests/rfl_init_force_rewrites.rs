//! c02 §A4 + owner-judgment item 7 — `rfl init --yes --force` rewrites a
//! hand-edited lock from defaults and rewrites the package dir for the
//! bundled openai plugin.

mod common;

use std::fs;
use std::process::Command;

use common::workspace_bin_path::workspace_bin;
use rafaello_core::topic_id;

const OPENAI_CANONICAL: &str = "builtin:openai@0.0.0";

#[test]
fn rfl_init_force_rewrites() {
    let project = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    setup_bundled_openai(bundled.path());

    let lock_path = project.path().join("rafaello.lock");
    fs::write(
        &lock_path,
        "# hand-edited\n[plugin.\"hand-edit:foo@0.0.0\"]\nentry = \"bin/x\"\n",
    )
    .unwrap();

    let topic = topic_id::derive(OPENAI_CANONICAL);
    let plugin_dir = project
        .path()
        .join(".rafaello")
        .join("plugins")
        .join(&topic);
    fs::create_dir_all(&plugin_dir).unwrap();
    let sentinel = plugin_dir.join("stale-sentinel.txt");
    fs::write(&sentinel, b"stale").unwrap();

    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .arg("init")
        .arg("--yes")
        .arg("--force")
        .arg("--project-root")
        .arg(project.path())
        .env("RFL_BUNDLED_PLUGINS_DIR", bundled.path())
        .env("RFL_BUNDLED_BIN_OPENAI", workspace_bin("rfl-openai-stub"))
        .output()
        .expect("spawn rfl init");
    assert!(
        out.status.success(),
        "rfl init --force failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let lock_after = fs::read_to_string(&lock_path).unwrap();
    assert!(
        !lock_after.contains("hand-edit:foo"),
        "garbage plugin entry survived --force rewrite: {lock_after}"
    );
    assert!(
        lock_after.contains(OPENAI_CANONICAL),
        "openai plugin entry missing after --force: {lock_after}"
    );

    assert!(
        !sentinel.exists(),
        "stale package-dir sentinel survived --force rewrite"
    );
    assert!(
        plugin_dir.join("rafaello.toml").is_file(),
        "package-dir manifest missing after --force rewrite"
    );
}

fn setup_bundled_openai(root: &std::path::Path) {
    let plugin_dir = root.join("openai");
    fs::create_dir_all(plugin_dir.join("bin")).unwrap();
    fs::write(
        plugin_dir.join("rafaello.toml"),
        r#"schema = 1
name = "openai"
version = "0.0.0"
entry = "bin/rfl-openai"
rafaello = ">=0.1, <0.2"
load = "eager"

[provides]
provider = "openai"

[bus]
subscribes = ["core.session.user_message"]
publishes = ["provider.openai.assistant_message"]

[capabilities.default.network]
mode = "proxy"
allow_hosts = ["litellm.example"]

[capabilities.default.env]
pass = []
allow_secrets = ["OPENAI_API_KEY"]
"#,
    )
    .unwrap();
    fs::write(
        plugin_dir.join("bin").join("rfl-openai"),
        "#!/bin/sh\nexec \"$@\"\n",
    )
    .unwrap();
}
