//! c28 / scope §C38c + m5a retro §5 item 15 — positive half of m5a's c38
//! dispatch cutover: a real `core.session.tool_request` flows through the
//! gate (gate-decided allow via a matching `user_grants` entry) →
//! `plugin.<id>.tool_request` → the bundled `rafaello-fetch` handler →
//! `core.session.tool_result`, end-to-end.
//!
//! Drives `rfl chat` against an m5b-shaped lock with:
//!
//! - `RFL_TUI_TEST_GRANT_BEFORE_MESSAGE` pre-grants the exact web-fetch
//!   URL via the c37-shipped synthetic-`/grant` slash command (exact-value
//!   `args_subset` subset matching mirrors live `UserGrants::matches`).
//! - The in-process OpenAI stub scripts a single `web-fetch` tool_call
//!   against that same URL.
//!
//! Asserts the gate's grant-match short-circuit drove dispatch:
//!
//! - **Zero** `confirm_request` audit rows (no modal fired).
//! - `fetch.log` records exactly one `web-fetch: <url>` line (the
//!   dispatch actually reached the plugin handler).

#![cfg(target_os = "linux")]

mod common;

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, Utc};
use common::m5a_demo_kit::{audit_kinds, OpenAiStub};
use common::workspace_bin_path::workspace_bin;
use rafaello_core::digest;
use rafaello_core::lock::{
    Bindings, CanonicalId, Grant, GrantBundle, GrantEnv, GrantFilesystem, GrantNetwork, LoadPolicy,
    Lock, LockFlags, PluginEntry, SessionTable, ToolMeta,
};
use rafaello_core::manifest::capabilities::NetworkMode;
use rafaello_core::manifest::{Manifest, SafePath};
use rafaello_core::topic_id;
use rusqlite::Connection;
use serde_json::{json, Value};
use serial_test::serial;

const OPENAI_CANONICAL: &str = "builtin:openai@0.0.0";
const FETCH_CANONICAL: &str = "local:rafaello-fetch@0.0.0";

const TARGET_URL: &str = "https://content.example.com/page";

#[test]
#[serial(rfl_chat)]
fn rfl_chat_tool_dispatch_goes_through_gate() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");
    let _ = workspace_bin("rfl-openai");
    let _ = workspace_bin("rafaello-fetch");

    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path();

    let stub = OpenAiStub::start(stub_responses());
    let endpoint = stub.endpoint();

    install_m5b_c28_lock(project_root, &endpoint);

    let body_file = tmp.path().join("body.txt");
    std::fs::write(&body_file, "fixture body for c28 positive").unwrap();

    let fetch_private_state = project_root
        .join(".rafaello-plugin-data")
        .join(topic_id::derive(FETCH_CANONICAL));
    std::fs::create_dir_all(&fetch_private_state).expect("mkdir fetch private state");
    let fetch_log_path = fetch_private_state.join("fetch.log");

    let grant_before = json!({
        "tool": "web-fetch",
        "args_subset": {"url": TARGET_URL},
    })
    .to_string();

    let output = Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(project_root)
        .env("RFL_TUI_TEST_MODE", "1")
        .env("RFL_TUI_PATH", workspace_bin("rfl-tui"))
        .env(
            "RFL_TUI_TEST_MESSAGE",
            "please fetch content.example.com/page",
        )
        .env("RFL_TUI_TEST_GRANT_BEFORE_MESSAGE", &grant_before)
        .env("RFL_FETCH_TEST_BODY_PATH", &body_file)
        .env("RFL_FETCH_TEST_LOG_PATH", &fetch_log_path)
        .env("RFL_TUI_MAX_LIFETIME", "10")
        .env("LITELLM_API_KEY", "sk-test-c28")
        .output()
        .expect("spawn rfl chat");

    drop(stub);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected zero exit; stderr={stderr}"
    );

    let state_dir = project_root.join(".rafaello").join("state");
    let conn = Connection::open(state_dir.join("session.sqlite")).expect("open audit sqlite");
    let kinds = audit_kinds(&conn);
    assert!(
        !kinds.iter().any(|k| k == "confirm_request"),
        "grant-match short-circuit must drive dispatch — no confirm_request \
         rows expected; got {kinds:?}; stderr={stderr}"
    );

    let fetch_log = std::fs::read_to_string(&fetch_log_path)
        .unwrap_or_else(|e| panic!("read fetch.log at {fetch_log_path:?}: {e}; stderr={stderr}"));
    let fetch_lines: Vec<&str> = fetch_log.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(
        fetch_lines.len(),
        1,
        "fetch.log should record exactly one invocation; got {fetch_lines:#?}; stderr={stderr}"
    );
    assert!(
        fetch_lines[0].contains(TARGET_URL),
        "fetch.log entry references the dispatched URL; got {:?}",
        fetch_lines[0]
    );
}

fn stub_responses() -> Value {
    json!([
        {
            "id": "cmpl-tool",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_web_fetch_1",
                        "type": "function",
                        "function": {
                            "name": "web-fetch",
                            "arguments": format!("{{\"url\":\"{TARGET_URL}\"}}")
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        },
        {
            "id": "cmpl-final",
            "choices": [{
                "index": 0,
                "message": { "role": "assistant", "content": "done" },
                "finish_reason": "stop"
            }]
        }
    ])
}

fn install_m5b_c28_lock(project_root: &Path, openai_endpoint: &str) {
    let install_root = project_root.join(".rafaello").join("plugins");

    let openai = install_plugin(
        &install_root,
        &m5b_fixture_dir("rafaello-openai"),
        OPENAI_CANONICAL,
        "bin/rfl-openai",
        Some(&workspace_bin("rfl-openai")),
        |entry| {
            entry.bindings.provider = true;
            entry.bindings.provider_id = Some("openai".to_string());
            entry.bindings.load = LoadPolicy::Eager;
            entry.grant.subscribes = vec![
                "core.session.user_message".to_string(),
                "core.session.tool_result".to_string(),
            ];
            entry.grant.publishes = vec![
                "provider.openai.tool_request".to_string(),
                "provider.openai.assistant_message".to_string(),
            ];
            let mut env_set = BTreeMap::new();
            env_set.insert(
                "RFL_OPENAI_API_KEY_ENV".to_string(),
                "LITELLM_API_KEY".to_string(),
            );
            env_set.insert(
                "RFL_OPENAI_ENDPOINT_URL".to_string(),
                openai_endpoint.to_string(),
            );
            env_set.insert(
                "RFL_OPENAI_MODEL".to_string(),
                "vllm/qwen3.6-27b".to_string(),
            );
            entry.grant.bundles.insert(
                "default".to_string(),
                GrantBundle {
                    filesystem: Some(GrantFilesystem {
                        exec_dirs: runtime_exec_dirs(),
                        ..GrantFilesystem::default()
                    }),
                    network: Some(GrantNetwork {
                        mode: NetworkMode::AllowAll,
                        allow_hosts: Vec::new(),
                    }),
                    env: Some(GrantEnv {
                        pass: vec!["LITELLM_API_KEY".to_string()],
                        set: env_set,
                        allow_secrets: vec!["LITELLM_API_KEY".to_string()],
                    }),
                    ..GrantBundle::default()
                },
            );
        },
    );

    let fetch = install_plugin(
        &install_root,
        &m5b_fixture_dir("rafaello-fetch"),
        FETCH_CANONICAL,
        "bin/rafaello-fetch",
        Some(&workspace_bin("rafaello-fetch")),
        |entry| {
            entry.bindings.tools = vec!["web-fetch".to_string()];
            entry.bindings.load = LoadPolicy::Eager;
            entry.bindings.tool_meta.insert(
                "web-fetch".to_string(),
                ToolMeta {
                    sinks: vec!["network".to_string()],
                    sinks_inferred: false,
                    grant_match: Some(SafePath::parse("schemas/web-fetch-grant.json").unwrap()),
                    always_confirm: false,
                },
            );
            entry.grant.bundles.insert(
                "default".to_string(),
                GrantBundle {
                    filesystem: Some(GrantFilesystem {
                        exec_dirs: runtime_exec_dirs(),
                        ..GrantFilesystem::default()
                    }),
                    network: Some(GrantNetwork {
                        mode: NetworkMode::AllowAll,
                        allow_hosts: Vec::new(),
                    }),
                    env: Some(GrantEnv {
                        pass: vec![
                            "RFL_FETCH_TEST_BODY_PATH".to_string(),
                            "RFL_FETCH_TEST_LOG_PATH".to_string(),
                        ],
                        set: BTreeMap::new(),
                        allow_secrets: Vec::new(),
                    }),
                    ..GrantBundle::default()
                },
            );
        },
    );

    let mut plugins: BTreeMap<CanonicalId, PluginEntry> = BTreeMap::new();
    plugins.insert(openai.0, openai.1);
    plugins.insert(fetch.0, fetch.1);
    let lock = Lock {
        plugins,
        session: SessionTable {
            provider_active: Some(OPENAI_CANONICAL.to_string()),
            tool_owner: BTreeMap::new(),
        },
    };
    std::fs::write(project_root.join("rafaello.lock"), lock.to_toml())
        .expect("write rafaello.lock");
}

fn m5b_fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("m5b-locks")
        .join(name)
}

fn install_plugin(
    install_root: &Path,
    fixture_dir: &Path,
    canonical_str: &str,
    entry_rel: &str,
    real_binary: Option<&Path>,
    customise: impl FnOnce(&mut PluginEntry),
) -> (CanonicalId, PluginEntry) {
    let canonical = CanonicalId::parse(canonical_str).expect("canonical id");
    let topic = topic_id::derive(canonical_str);
    let plugin_dir = install_root.join(&topic);
    copy_dir_all(fixture_dir, &plugin_dir);

    let entry_abs = plugin_dir.join(entry_rel);
    if let Some(src) = real_binary {
        std::fs::copy(src, &entry_abs).expect("copy real plugin binary");
    }
    std::fs::set_permissions(&entry_abs, std::fs::Permissions::from_mode(0o755))
        .expect("chmod entry");

    let manifest_raw =
        std::fs::read_to_string(plugin_dir.join("rafaello.toml")).expect("read fixture manifest");
    let manifest = Manifest::parse(&manifest_raw).expect("parse manifest");
    let manifest_digest = digest::manifest_digest(&manifest.canonical_bytes());
    let content_digest = digest::content_digest(&plugin_dir).expect("content_digest");

    let granted_at: DateTime<Utc> = "2026-05-11T00:00:00Z".parse().unwrap();
    let mut entry = PluginEntry {
        entry: SafePath::parse(entry_rel).expect("safepath"),
        digest: content_digest,
        manifest_digest,
        granted_at,
        grant: Grant::default(),
        bindings: Bindings::default(),
        flags: LockFlags::default(),
    };
    customise(&mut entry);
    (canonical, entry)
}

fn runtime_exec_dirs() -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    if let Some(val) = std::env::var_os("LOCKIN_TEST_EXEC_DIRS") {
        for d in std::env::split_paths(&val) {
            if d.is_absolute() {
                out.push(d.to_string_lossy().into_owned());
            }
        }
    }
    if out.is_empty() {
        out.push("/nix/store".to_string());
    }
    out
}

fn copy_dir_all(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("create dst dir");
    for ent in std::fs::read_dir(src).expect("read src dir") {
        let ent = ent.expect("dir entry");
        let from = ent.path();
        let to = dst.join(ent.file_name());
        let ft = ent.file_type().expect("file type");
        if ft.is_dir() {
            copy_dir_all(&from, &to);
        } else {
            std::fs::copy(&from, &to).expect("copy file");
        }
    }
}
