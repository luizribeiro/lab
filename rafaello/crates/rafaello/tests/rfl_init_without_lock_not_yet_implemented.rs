//! c02 (amended from c01) — `rfl init` from a cwd without a pre-existing
//! lock now exits 0 and materialises the default lock (two-stage ladder
//! per m0 §4.3 — the c01 `NotYetImplemented` assertion is amended away).

mod common;

use std::fs;
use std::process::Command;

use common::workspace_bin_path::workspace_bin;

#[test]
fn rfl_init_without_lock_now_succeeds() {
    let project = tempfile::tempdir().unwrap();
    let bundled = tempfile::tempdir().unwrap();
    let openai = bundled.path().join("openai").join("bin");
    fs::create_dir_all(&openai).unwrap();
    fs::write(
        bundled.path().join("openai").join("rafaello.toml"),
        r#"schema = 1
name = "openai"
version = "0.0.0"
entry = "bin/rfl-openai"
rafaello = ">=0.1, <0.2"
load = "eager"

[provides]
provider = "openai"

[capabilities.default.network]
mode = "proxy"
allow_hosts = ["litellm.example"]

[capabilities.default.env]
pass = []
allow_secrets = ["OPENAI_API_KEY"]
"#,
    )
    .unwrap();
    fs::write(openai.join("rfl-openai"), "#!/bin/sh\nexec \"$@\"\n").unwrap();

    let rfl = workspace_bin("rfl");
    let out = Command::new(rfl)
        .arg("init")
        .arg("--yes")
        .arg("--project-root")
        .arg(project.path())
        .env("RFL_BUNDLED_PLUGINS_DIR", bundled.path())
        .env("RFL_BUNDLED_BIN_OPENAI", workspace_bin("rfl-openai-stub"))
        .output()
        .expect("spawn rfl init");

    assert!(
        out.status.success(),
        "rfl init should now succeed (c02): stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(project.path().join("rafaello.lock").is_file());
}
