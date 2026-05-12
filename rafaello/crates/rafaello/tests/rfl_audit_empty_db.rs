//! c11 (scope §D3) — after `rfl init --yes --project-root <tmpdir>`
//! (which does not create an audit DB), `rfl audit --project-root
//! <tmpdir>` exits 0 with stderr containing `"no audit events"`.
//!
//! Depends on c02 (pi-1 M-2 `rfl init` fresh-lock baseline).

mod common;

use std::fs;
use std::path::Path;
use std::process::Command;

use common::workspace_bin_path::workspace_bin;

#[test]
fn rfl_audit_empty_db() {
    let project = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    setup_bundled_openai(bundled.path());

    let rfl = workspace_bin("rfl");
    let init = Command::new(&rfl)
        .args(["init", "--yes", "--project-root"])
        .arg(project.path())
        .env("RFL_BUNDLED_PLUGINS_DIR", bundled.path())
        .output()
        .expect("spawn rfl init");
    assert!(
        init.status.success(),
        "rfl init failed: stderr={}",
        String::from_utf8_lossy(&init.stderr)
    );

    let audit = Command::new(&rfl)
        .args(["audit", "--project-root"])
        .arg(project.path())
        .output()
        .expect("spawn rfl audit");
    assert!(
        audit.status.success(),
        "rfl audit failed: stderr={}",
        String::from_utf8_lossy(&audit.stderr)
    );
    let stdout = String::from_utf8_lossy(&audit.stdout);
    assert!(
        stdout.trim().is_empty(),
        "expected empty stdout, got: {stdout}"
    );
    let stderr = String::from_utf8_lossy(&audit.stderr);
    assert!(
        stderr.contains("no audit events"),
        "expected stderr to mention 'no audit events', got: {stderr}"
    );
}

fn setup_bundled_openai(root: &Path) {
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
