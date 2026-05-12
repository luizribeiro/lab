//! c26 / scope §C38a + m5a retro §5 item 12 — five-tree eager-spawn
//! smoke test against the m5b fixture lock.
//!
//! Materialises a lock with FIVE plugins (active openai + inactive
//! mockprovider + rafaello-fetch + rafaello-mailcat + rafaello-readfile)
//! and spawns `rfl chat`. Once the TUI signals frontend-ready (which
//! happens only after every eager plugin has spawned — lib.rs steps
//! C6–C10), the test SIGINTs the parent and asserts:
//!   * `rfl chat` exits cleanly (status 0) — every child observation
//!     reached the supervisor's shutdown reaper without a crash.
//!   * `.rafaello-plugin-data/<topic_id>/` exists for all five canonical
//!     ids — each plugin progressed through the supervisor's
//!     pre-spawn `create_dir_all` of `private_state_dir`.
//!
//! Per pi-1 B-4 this commit does NOT mutate the c22 m5b fixture lock.

#![cfg(target_os = "linux")]

mod common;

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use common::workspace_bin_path::workspace_bin;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use rafaello_core::digest;
use rafaello_core::lock::{
    Bindings, CanonicalId, Grant, GrantBundle, GrantEnv, GrantFilesystem, GrantNetwork, LoadPolicy,
    Lock, LockFlags, PluginEntry, SessionTable, ToolMeta,
};
use rafaello_core::manifest::capabilities::NetworkMode;
use rafaello_core::manifest::{Manifest, SafePath};
use rafaello_core::topic_id;
use serial_test::serial;

const OPENAI_CANONICAL: &str = "builtin:openai@0.0.0";
const MAILCAT_CANONICAL: &str = "local:mailcat@0.0.0";
const FETCH_CANONICAL: &str = "local:rafaello-fetch@0.0.0";
const MOCKPROVIDER_CANONICAL: &str = "local:mockprovider@0.0.0";
const READFILE_CANONICAL: &str = "local:readfile@0.0.0";

const FIVE_CANONICALS: &[&str] = &[
    OPENAI_CANONICAL,
    MAILCAT_CANONICAL,
    FETCH_CANONICAL,
    MOCKPROVIDER_CANONICAL,
    READFILE_CANONICAL,
];

#[test]
#[serial(rfl_chat)]
fn rfl_chat_eager_spawns_five_tree_and_shuts_down_cleanly() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");
    let _ = workspace_bin("rfl-openai");
    let _ = workspace_bin("rfl-mailcat");
    let _ = workspace_bin("rafaello-fetch");
    let _ = workspace_bin("rfl-mockprovider");
    let _ = workspace_bin("rfl-readfile");

    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path();

    install_m5b_five_plugin_lock(project_root);

    let mut child = Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(project_root)
        .env("RFL_TUI_TEST_MODE", "1")
        .env("RFL_TUI_PATH", workspace_bin("rfl-tui"))
        .env("RFL_TUI_MAX_LIFETIME", "30")
        .env("LITELLM_API_KEY", "sk-test-five-tree")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rfl chat");

    let stderr = child.stderr.take().expect("stderr piped");
    let (tx, rx) = mpsc::channel::<String>();
    let reader_handle = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let deadline = Instant::now() + Duration::from_secs(30);
    let mut captured: Vec<String> = Vec::new();
    let mut signalled = false;

    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(line) => {
                captured.push(line.clone());
                if line.contains("rfl-chat: frontend-ready-observed") {
                    let _ = signal::kill(Pid::from_raw(child.id() as i32), Signal::SIGINT);
                    signalled = true;
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => break,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    assert!(
        signalled,
        "did not observe `frontend-ready-observed` within 30s; stderr so far:\n{}",
        captured.join("\n")
    );

    let status = child.wait().expect("rfl chat wait");
    drop(rx);
    let _ = reader_handle.join();

    assert!(
        status.success(),
        "rfl chat should exit 0 after SIGINT (clean five-tree shutdown); status={status:?}; \
         stderr:\n{}",
        captured.join("\n")
    );

    let plugin_data_root = project_root.join(".rafaello-plugin-data");
    for canonical in FIVE_CANONICALS {
        let topic = topic_id::derive(canonical);
        let private_dir = plugin_data_root.join(&topic);
        assert!(
            private_dir.is_dir(),
            "expected `.rafaello-plugin-data/{topic}` for {canonical} after eager spawn; \
             plugin_data_root={plugin_data_root:?}; stderr:\n{}",
            captured.join("\n")
        );
    }
}

fn install_m5b_five_plugin_lock(project_root: &Path) {
    let install_root = project_root.join(".rafaello").join("plugins");

    let openai_entry = install_plugin(
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
                "http://127.0.0.1:1/v1".to_string(),
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

    let mailcat_entry = install_plugin(
        &install_root,
        &m5b_fixture_dir("rafaello-mailcat"),
        MAILCAT_CANONICAL,
        "bin/rfl-mailcat",
        Some(&workspace_bin("rfl-mailcat")),
        |entry| {
            entry.bindings.tools = vec!["send-mail".to_string()];
            entry.bindings.load = LoadPolicy::Eager;
            entry.bindings.tool_meta.insert(
                "send-mail".to_string(),
                ToolMeta {
                    sinks: vec!["mail".to_string()],
                    sinks_inferred: false,
                    grant_match: Some(SafePath::parse("schemas/send-mail-grant.json").unwrap()),
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
                    ..GrantBundle::default()
                },
            );
        },
    );

    let fetch_entry = install_plugin(
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
                        read_dirs: vec!["${project}".to_string()],
                        ..GrantFilesystem::default()
                    }),
                    network: Some(GrantNetwork {
                        mode: NetworkMode::AllowAll,
                        allow_hosts: Vec::new(),
                    }),
                    ..GrantBundle::default()
                },
            );
        },
    );

    let mockprovider_entry = install_plugin(
        &install_root,
        &m5b_fixture_dir("rafaello-mockprovider"),
        MOCKPROVIDER_CANONICAL,
        "bin/rfl-mockprovider",
        Some(&workspace_bin("rfl-mockprovider")),
        |entry| {
            entry.bindings.provider = true;
            entry.bindings.provider_id = Some("mock".to_string());
            entry.bindings.load = LoadPolicy::Eager;
            entry.grant.subscribes = vec![
                "core.session.user_message".to_string(),
                "core.session.tool_result".to_string(),
            ];
            entry.grant.publishes = vec![
                "provider.mock.tool_request".to_string(),
                "provider.mock.assistant_message".to_string(),
            ];
            entry.grant.bundles.insert(
                "default".to_string(),
                GrantBundle {
                    filesystem: Some(GrantFilesystem {
                        exec_dirs: runtime_exec_dirs(),
                        ..GrantFilesystem::default()
                    }),
                    network: Some(GrantNetwork {
                        mode: NetworkMode::Deny,
                        allow_hosts: Vec::new(),
                    }),
                    ..GrantBundle::default()
                },
            );
        },
    );

    let readfile_entry = install_plugin(
        &install_root,
        &m5b_fixture_dir("rafaello-readfile"),
        READFILE_CANONICAL,
        "bin/rfl-readfile",
        Some(&workspace_bin("rfl-readfile")),
        |entry| {
            entry.bindings.tools = vec!["read-file".to_string()];
            entry.bindings.load = LoadPolicy::Eager;
            entry.grant.bundles.insert(
                "default".to_string(),
                GrantBundle {
                    filesystem: Some(GrantFilesystem {
                        exec_dirs: runtime_exec_dirs(),
                        read_dirs: vec!["${project}".to_string()],
                        ..GrantFilesystem::default()
                    }),
                    network: Some(GrantNetwork {
                        mode: NetworkMode::Deny,
                        allow_hosts: Vec::new(),
                    }),
                    ..GrantBundle::default()
                },
            );
        },
    );

    let mut plugins = BTreeMap::new();
    plugins.insert(openai_entry.0, openai_entry.1);
    plugins.insert(mailcat_entry.0, mailcat_entry.1);
    plugins.insert(fetch_entry.0, fetch_entry.1);
    plugins.insert(mockprovider_entry.0, mockprovider_entry.1);
    plugins.insert(readfile_entry.0, readfile_entry.1);
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
