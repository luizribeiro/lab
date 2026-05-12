//! c24b / scope §I2 line 1224 — lazy-load tool trigger acceptance:
//! when a plugin's `bindings.load = { command = [...] }` registers it
//! as a lazy candidate, the first tool dispatch that matches a
//! trigger spawns the plugin on demand and writes exactly one
//! `spawn_on_demand <canonical>` line to the `RFL_SPAWN_TRACE_LOG`
//! file. Eager providers are recorded earlier on `eager_spawn` lines,
//! and a second dispatch through the same trigger does not produce a
//! second trace line (pi-5 B-4 idempotence via `managed.contains_key`).
//!
//! Pi-3 B-5 closure: parent integration tests cannot observe the
//! child's `PluginSupervisor` state directly; the `RFL_SPAWN_TRACE_LOG`
//! file-log mirrors `RFL_STARTUP_ORDERING_LOG`
//! (`rafaello/crates/rafaello/src/chat/test_ordering_hook.rs`) and
//! gives an inter-process observable channel without leaking
//! supervisor internals through a test seam.

#![cfg(target_os = "linux")]

mod common;

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use chrono::{DateTime, Utc};
use common::workspace_bin_path::workspace_bin;
use rafaello_core::digest;
use rafaello_core::lock::{
    Bindings, CanonicalId, Grant, GrantBundle, GrantFilesystem, GrantNetwork, LoadPolicy, Lock,
    LockFlags, PluginEntry, SessionTable,
};
use rafaello_core::manifest::capabilities::NetworkMode;
use rafaello_core::manifest::{Manifest, SafePath};
use rafaello_core::topic_id;
use serial_test::serial;

const MOCKPROVIDER_CANONICAL: &str = "local:mockprovider@0.0.0";
const READFILE_CANONICAL: &str = "local:readfile@0.0.0";

const README_BODY: &str = "m4 demo readme\n";

#[test]
#[serial(rfl_chat)]
fn lazy_load_tool_trigger_spawns_on_first_call() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");
    let _ = workspace_bin("rfl-mockprovider");
    let _ = workspace_bin("rfl-readfile");

    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path();
    std::fs::write(project_root.join("README.md"), README_BODY).expect("write README.md");
    install_lazy_readfile_lock(project_root);

    let trace_log = tmp.path().join("spawn-trace.log");

    let output = Command::new(workspace_bin("rfl"))
        .arg("chat")
        .arg("--project-root")
        .arg(project_root)
        .env("RFL_TUI_TEST_MODE", "1")
        .env("RFL_TUI_PATH", workspace_bin("rfl-tui"))
        .env("RFL_TUI_TEST_MESSAGE", "what's in README.md")
        .env("RFL_TUI_MAX_LIFETIME", "10")
        .env("RFL_SPAWN_TRACE_LOG", &trace_log)
        .output()
        .expect("spawn rfl chat");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "expected clean rfl chat exit; status={:?}; stderr=\n{stderr}",
        output.status
    );

    let trace =
        std::fs::read_to_string(&trace_log).expect("RFL_SPAWN_TRACE_LOG file must exist after run");
    let events: Vec<&str> = trace.lines().collect();

    let eager_provider_line = format!("eager_spawn {MOCKPROVIDER_CANONICAL}");
    let spawn_on_demand_line = format!("spawn_on_demand {READFILE_CANONICAL}");
    let lazy_eager_line = format!("eager_spawn {READFILE_CANONICAL}");

    let eager_pos = events
        .iter()
        .position(|e| *e == eager_provider_line)
        .unwrap_or_else(|| {
            panic!(
                "expected `{eager_provider_line}` in trace; got events={events:?}; stderr=\n{stderr}"
            )
        });
    let on_demand_pos = events
        .iter()
        .position(|e| *e == spawn_on_demand_line)
        .unwrap_or_else(|| {
            panic!(
                "expected `{spawn_on_demand_line}` in trace; got events={events:?}; stderr=\n{stderr}"
            )
        });

    assert!(
        eager_pos < on_demand_pos,
        "eager provider must spawn strictly before the lazy plugin; \
         eager_pos={eager_pos} on_demand_pos={on_demand_pos} events={events:?}"
    );

    assert!(
        !events.iter().any(|e| *e == lazy_eager_line),
        "lazy plugin must not appear on an `eager_spawn` line; events={events:?}"
    );

    let readfile_lines: Vec<&&str> = events
        .iter()
        .filter(|e| e.ends_with(READFILE_CANONICAL))
        .collect();
    assert_eq!(
        readfile_lines.len(),
        1,
        "lazy plugin must appear exactly once in trace (pi-5 B-4 idempotence); \
         readfile_lines={readfile_lines:?} events={events:?}"
    );
}

fn install_lazy_readfile_lock(project_root: &Path) {
    let install_root = project_root.join(".rafaello").join("plugins");

    let mockprovider_entry = install_plugin(
        &install_root,
        &m5b_fixture_dir("rafaello-mockprovider"),
        MOCKPROVIDER_CANONICAL,
        "bin/rfl-mockprovider",
        &workspace_bin("rfl-mockprovider"),
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
                        mode: NetworkMode::AllowAll,
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
        &workspace_bin("rfl-readfile"),
        |entry| {
            entry.bindings.tools = vec!["read-file".to_string()];
            entry.bindings.load = LoadPolicy::Lazy {
                event: Vec::new(),
                command: vec!["read-file".to_string()],
                kind: Vec::new(),
            };
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
    plugins.insert(mockprovider_entry.0, mockprovider_entry.1);
    plugins.insert(readfile_entry.0, readfile_entry.1);
    let lock = Lock {
        plugins,
        session: SessionTable {
            provider_active: Some(MOCKPROVIDER_CANONICAL.to_string()),
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
    real_binary: &Path,
    customise: impl FnOnce(&mut PluginEntry),
) -> (CanonicalId, PluginEntry) {
    let canonical = CanonicalId::parse(canonical_str).expect("canonical id");
    let topic = topic_id::derive(canonical_str);
    let plugin_dir = install_root.join(&topic);
    copy_dir_all(fixture_dir, &plugin_dir);

    let entry_abs = plugin_dir.join(entry_rel);
    std::fs::copy(real_binary, &entry_abs).expect("copy real plugin binary");
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
