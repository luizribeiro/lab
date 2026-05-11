//! c40: shared scaffolding for the demo-bar negative integration tests
//! (timeout / always_allow_session restart / always_confirm non-sink).
//! c39 (`rfl_chat_demo_bar_send_mail.rs`) was authored before this kit
//! existed and keeps its own inlined copy of the same setup — the kit
//! is c40-internal scaffolding only and not a refactor of c39.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
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

use super::workspace_bin_path::workspace_bin;

pub const OPENAI_CANONICAL: &str = "builtin:openai@0.0.0";
pub const MAILCAT_CANONICAL: &str = "local:mailcat@0.0.0";
pub const ALICE: &str = "alice@example.com";

/// Override knobs for the mailcat plugin's `send-mail` `tool_meta` in
/// the materialised lock.
pub struct MailcatToolMetaOverrides {
    pub sinks: Vec<String>,
    pub always_confirm: bool,
    pub grant_match_path: Option<&'static str>,
}

impl Default for MailcatToolMetaOverrides {
    fn default() -> Self {
        Self {
            sinks: vec!["mail".to_string()],
            always_confirm: false,
            grant_match_path: Some("schemas/send-mail-grant.json"),
        }
    }
}

/// In-process stub of `rfl-openai-stub`'s recorded-response contract
/// (scope §A8). Mirrors the implementation inlined in c39's
/// `rfl_chat_demo_bar_send_mail.rs`.
pub struct OpenAiStub {
    pub port: u16,
    shutdown: Arc<Mutex<bool>>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl OpenAiStub {
    pub fn start(responses: Value) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub listener");
        listener
            .set_nonblocking(true)
            .expect("set listener nonblocking");
        let port = listener.local_addr().expect("local_addr").port();
        let responses: Vec<Value> = responses.as_array().cloned().unwrap_or_default();
        assert!(
            !responses.is_empty(),
            "stub responses array must be non-empty"
        );
        let shutdown = Arc::new(Mutex::new(false));
        let s = shutdown.clone();
        let join = std::thread::spawn(move || {
            let mut next = 0usize;
            loop {
                if *s.lock().unwrap() {
                    return;
                }
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                        let pick = &responses[next.min(responses.len() - 1)];
                        next += 1;
                        let _ = serve_one(&mut stream, pick);
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    Err(_) => return,
                }
            }
        });
        Self {
            port,
            shutdown,
            join: Some(join),
        }
    }

    pub fn endpoint(&self) -> String {
        format!("http://127.0.0.1:{}/v1", self.port)
    }
}

impl Drop for OpenAiStub {
    fn drop(&mut self) {
        *self.shutdown.lock().unwrap() = true;
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

fn serve_one(stream: &mut std::net::TcpStream, response_body: &Value) -> std::io::Result<()> {
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let mut content_length = 0usize;
    let mut head_end = None;
    while head_end.is_none() {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(idx) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            head_end = Some(idx + 4);
            let head = std::str::from_utf8(&buf[..idx]).unwrap_or("");
            for line in head.split("\r\n") {
                let lower = line.to_ascii_lowercase();
                if let Some(rest) = lower.strip_prefix("content-length:") {
                    content_length = rest.trim().parse().unwrap_or(0);
                }
            }
        }
    }
    let head_end = head_end.unwrap();
    while buf.len() < head_end + content_length {
        let n = stream.read(&mut chunk)?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = serde_json::to_vec(response_body).unwrap_or_default();
    let head = format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&body)?;
    stream.flush()?;
    Ok(())
}

/// Two-turn recorded response: tool_call (`send-mail to=alice@…`) then
/// a final text reply.
pub fn stub_send_mail_then_text(final_text: &str) -> Value {
    json!([
        {
            "id": "cmpl-tool",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_send_mail_1",
                        "type": "function",
                        "function": {
                            "name": "send-mail",
                            "arguments": "{\"to\":\"alice@example.com\"}"
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
                "message": { "role": "assistant", "content": final_text },
                "finish_reason": "stop"
            }]
        }
    ])
}

pub fn mailcat_log_path(project_root: &Path) -> PathBuf {
    project_root
        .join(".rafaello-plugin-data")
        .join(topic_id::derive(MAILCAT_CANONICAL))
        .join("mailcat.log")
}

pub fn audit_kinds(conn: &Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT kind FROM audit_events ORDER BY seq")
        .expect("prepare audit query");
    stmt.query_map([], |r| r.get::<_, String>(0))
        .expect("query audit")
        .filter_map(Result::ok)
        .collect()
}

pub fn audit_rows(conn: &Connection) -> Vec<(String, String)> {
    let mut stmt = conn
        .prepare("SELECT kind, payload FROM audit_events ORDER BY seq")
        .expect("prepare audit query");
    stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .expect("query audit")
        .filter_map(Result::ok)
        .collect()
}

/// Materialise the m5a demo-bar lock at `${project_root}/rafaello.lock`
/// plus the openai + mailcat install dirs under
/// `${project_root}/.rafaello/plugins/<topic_id>/`.
pub fn install_m5a_demo_lock(
    project_root: &Path,
    openai_endpoint: &str,
    mailcat_overrides: MailcatToolMetaOverrides,
) {
    let install_root = project_root.join(".rafaello").join("plugins");

    let openai_entry = install_plugin(
        &install_root,
        &m5a_fixture_dir("rafaello-openai"),
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

    let mailcat_entry = install_plugin(
        &install_root,
        &m5a_fixture_dir("rafaello-mailcat"),
        MAILCAT_CANONICAL,
        "bin/rfl-mailcat",
        Some(&workspace_bin("rfl-mailcat")),
        |entry| {
            entry.bindings.tools = vec!["send-mail".to_string()];
            entry.bindings.load = LoadPolicy::Eager;
            entry.bindings.tool_meta.insert(
                "send-mail".to_string(),
                ToolMeta {
                    sinks: mailcat_overrides.sinks.clone(),
                    sinks_inferred: false,
                    grant_match: mailcat_overrides
                        .grant_match_path
                        .map(|p| SafePath::parse(p).expect("safepath grant_match")),
                    always_confirm: mailcat_overrides.always_confirm,
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

    let mut plugins = BTreeMap::new();
    plugins.insert(openai_entry.0, openai_entry.1);
    plugins.insert(mailcat_entry.0, mailcat_entry.1);
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

fn m5a_fixture_dir(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("m5a-locks")
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
