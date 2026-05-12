//! c27 — headline integrated demo (scope §"Demo bar" / §"Headline
//! integrated demo" + hard requirement #3 / Phase J2). Programmatic
//! companion to the tmux-driven §5 transcripts under
//! `rafaello/plans/milestones/m6-polish-release/transcripts/section-5/`.
//!
//! Drives the full `rfl init → rfl install rfl-mailcat → rfl chat →
//! confirm → persist` flow against a synthetic bundled-plugins root
//! whose `openai` manifest's `[capabilities.default.env.set]` table
//! points `RFL_OPENAI_ENDPOINT_URL` at an in-process stub mirroring
//! `rfl-openai-stub`'s scripted-turns contract (scope §A8 / §E1).
//! `RFL_TUI_TEST_CONFIRM_ANSWERS=allow` (m5b §5 row 56 hook) holds
//! the confirmation arm steady. Asserts the `entries` table holds
//! the canonical tool_call + tool_result + assistant-message rows
//! and the `audit_events` table holds the `confirm_request` +
//! `confirm_allowed` rows. The chat process is required to exit
//! cleanly (the round-3 J2 quit binding lands on `Ctrl-C` in the
//! live TUI; under `RFL_TUI_TEST_MODE=1` the chat exits after the
//! scripted turns drain — `output.status.success()` is the
//! regression-grade analogue of "clean exit on `Ctrl-C`").

mod common;

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::install_test_kit::copy_in_tree_to_bundled_dir;
use common::workspace_bin_path::workspace_bin;
use rafaello_core::session::SessionStore;
use rafaello_core::topic_id;
use rusqlite::Connection;
use serde_json::{json, Value};
use serial_test::serial;

const MAILCAT_CANONICAL: &str = "local:mailcat@0.0.0";
const ALICE: &str = "alice@example.com";

#[test]
#[serial(rfl_chat)]
fn rfl_chat_demo_bar_init_install_chat_confirm_persist() {
    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");
    let real_openai = workspace_bin("rfl-openai");
    let real_mailcat = workspace_bin("rfl-mailcat");

    let project = tempfile::tempdir().unwrap();
    let project_root = project.path();

    let final_text = "Email sent to alice.";
    let stub = OpenAiStub::start(stub_responses(final_text));
    let endpoint = format!("http://127.0.0.1:{}/v1", stub.port);

    let bundled = tempfile::tempdir().unwrap();
    write_openai_bundled_tree(&bundled.path().join("openai"), &endpoint, &real_openai);
    let mailcat_dir =
        copy_in_tree_to_bundled_dir(bundled.path(), "rfl-mailcat", "rafaello-mailcat");
    let mailcat_bin = mailcat_dir.join("bin").join("rfl-mailcat");
    fs::copy(&real_mailcat, &mailcat_bin).expect("copy real rfl-mailcat into bundled tree");
    fs::set_permissions(&mailcat_bin, fs::Permissions::from_mode(0o755)).unwrap();
    // The in-tree mailcat manifest declares no `[capabilities]`
    // table; under the live lockin sandbox the materialised plugin
    // binary needs an `exec_dirs` entry covering the project tree
    // so `syd` can exec it. Rewrite the bundled manifest with a
    // minimal `[capabilities.default.filesystem]` block before
    // `rfl install` reads it.
    rewrite_mailcat_manifest_with_exec_dirs(&mailcat_dir.join("rafaello.toml"));

    let rfl = workspace_bin("rfl");

    let init = Command::new(&rfl)
        .args(["init", "--yes", "--project-root"])
        .arg(project_root)
        .env("RFL_BUNDLED_PLUGINS_DIR", bundled.path())
        .output()
        .expect("spawn rfl init");
    assert!(
        init.status.success(),
        "rfl init failed: stderr={}",
        String::from_utf8_lossy(&init.stderr)
    );

    let install = Command::new(&rfl)
        .args(["install", "rfl-mailcat", "--project-root"])
        .arg(project_root)
        .env("RFL_BUNDLED_PLUGINS_DIR", bundled.path())
        .output()
        .expect("spawn rfl install");
    assert!(
        install.status.success(),
        "rfl install rfl-mailcat failed: stderr={}",
        String::from_utf8_lossy(&install.stderr)
    );

    let chat = Command::new(&rfl)
        .arg("chat")
        .arg("--project-root")
        .arg(project_root)
        .env("RFL_TUI_TEST_MODE", "1")
        .env("RFL_TUI_PATH", workspace_bin("rfl-tui"))
        .env(
            "RFL_TUI_TEST_MESSAGE",
            format!("Please email {ALICE} a one-line hello note."),
        )
        .env("RFL_TUI_TEST_CONFIRM_ANSWERS", "allow")
        .env("RFL_TUI_MAX_LIFETIME", "15")
        .env("LITELLM_API_KEY", "sk-c27-demo-bar")
        .output()
        .expect("spawn rfl chat");

    drop(stub);

    let chat_stderr = String::from_utf8_lossy(&chat.stderr);
    assert!(
        chat.status.success(),
        "expected clean exit (round-3 J2: `Ctrl-C` in the live TUI; \
         `output.status.success()` is the test-mode analogue); stderr={chat_stderr}"
    );

    let state_dir = project_root.join(".rafaello").join("state");
    let store = SessionStore::open(&state_dir).expect("open session store");
    let stored = store.load_entries().expect("load entries");

    let kinds: Vec<&str> = stored.iter().map(|s| s.entry.kind.as_str()).collect();
    assert!(
        kinds.contains(&"tool_call"),
        "entries missing tool_call row; kinds={kinds:?}\nstderr={chat_stderr}"
    );
    assert!(
        kinds.contains(&"tool_result"),
        "entries missing tool_result row; kinds={kinds:?}\nstderr={chat_stderr}"
    );
    assert!(
        kinds.contains(&"text"),
        "entries missing assistant-message text row; kinds={kinds:?}\nstderr={chat_stderr}"
    );

    let tool_call = stored
        .iter()
        .find(|s| s.entry.kind == "tool_call")
        .expect("tool_call row present");
    assert_eq!(tool_call.entry.payload["name"].as_str(), Some("send-mail"));
    assert_eq!(tool_call.entry.payload["args"]["to"].as_str(), Some(ALICE));

    let tool_result = stored
        .iter()
        .find(|s| s.entry.kind == "tool_result")
        .expect("tool_result row present");
    assert_eq!(tool_result.entry.payload["ok"].as_bool(), Some(true));

    let conn = Connection::open(state_dir.join("session.sqlite")).expect("open audit sqlite");
    let mut stmt = conn
        .prepare("SELECT kind FROM audit_events ORDER BY seq")
        .expect("prepare audit query");
    let audit_kinds: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .expect("query audit")
        .filter_map(Result::ok)
        .collect();
    assert!(
        audit_kinds.iter().any(|k| k == "confirm_request"),
        "audit missing confirm_request; got {audit_kinds:?}"
    );
    assert!(
        audit_kinds.iter().any(|k| k == "confirm_allowed"),
        "audit missing confirm_allowed; got {audit_kinds:?}"
    );

    let mailcat_log = project_root
        .join(".rafaello-plugin-data")
        .join(topic_id::derive(MAILCAT_CANONICAL))
        .join("mailcat.log");
    let log_raw = fs::read_to_string(&mailcat_log).expect("read mailcat.log");
    assert!(
        log_raw.contains(ALICE),
        "mailcat.log missing {ALICE}; raw={log_raw}"
    );

    let _ = final_text;
}

fn rewrite_mailcat_manifest_with_exec_dirs(manifest_path: &Path) {
    let raw = fs::read_to_string(manifest_path).expect("read mailcat manifest");
    let exec_dirs_toml = exec_dirs_toml_array();
    let appended = format!(
        "{raw}\n\
         [capabilities.default.filesystem]\n\
         exec_dirs = {exec_dirs_toml}\n\
         \n\
         [capabilities.default.network]\n\
         mode = \"allow_all\"\n"
    );
    fs::write(manifest_path, appended).expect("rewrite mailcat manifest");
}

/// `exec_dirs` array literal covering the test temp tree (the
/// materialised plugin binaries live under `tempfile::tempdir()` —
/// typically `/tmp/...` or `$TMPDIR/...`) plus `/nix/store` (the
/// devshell-provided lib closure). Validation refuses entries that
/// resolve inside `${project}`, so the project tempdir's *parent*
/// is what lands here.
fn exec_dirs_toml_array() -> String {
    let mut dirs: Vec<String> = Vec::new();
    if let Some(val) = std::env::var_os("LOCKIN_TEST_EXEC_DIRS") {
        for d in std::env::split_paths(&val) {
            if d.is_absolute() {
                dirs.push(d.to_string_lossy().into_owned());
            }
        }
    }
    let temp_root = std::env::temp_dir();
    let temp_root = temp_root.to_string_lossy().into_owned();
    if !dirs.iter().any(|d| d == &temp_root) {
        dirs.push(temp_root);
    }
    if !dirs.iter().any(|d| d == "/nix/store") {
        dirs.push("/nix/store".to_string());
    }
    let body: Vec<String> = dirs
        .into_iter()
        .map(|d| format!("\"{}\"", d.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();
    format!("[{}]", body.join(", "))
}

fn write_openai_bundled_tree(plugin_dir: &Path, endpoint: &str, real_bin: &Path) {
    fs::create_dir_all(plugin_dir.join("bin")).unwrap();
    let manifest = format!(
        r#"schema = 1
name = "openai"
version = "0.0.0"
entry = "bin/rfl-openai"
rafaello = ">=0.1, <0.2"
load = "eager"

[provides]
provider = "openai"

[bus]
subscribes = ["core.session.user_message", "core.session.tool_result"]
publishes = ["provider.openai.tool_request", "provider.openai.assistant_message"]

[capabilities.default.filesystem]
exec_dirs = {exec_dirs}

[capabilities.default.network]
mode = "allow_all"

[capabilities.default.env]
pass = ["LITELLM_API_KEY"]
allow_secrets = ["LITELLM_API_KEY"]

[capabilities.default.env.set]
RFL_OPENAI_API_KEY_ENV = "LITELLM_API_KEY"
RFL_OPENAI_ENDPOINT_URL = "{endpoint}"
RFL_OPENAI_MODEL = "vllm/qwen3.6-27b"
"#,
        endpoint = endpoint,
        exec_dirs = exec_dirs_toml_array(),
    );
    fs::write(plugin_dir.join("rafaello.toml"), manifest).unwrap();
    fs::write(plugin_dir.join("openrpc.json"), b"{}").unwrap();
    let entry = plugin_dir.join("bin").join("rfl-openai");
    fs::copy(real_bin, &entry).expect("copy real rfl-openai");
    fs::set_permissions(&entry, fs::Permissions::from_mode(0o755)).unwrap();
}

fn stub_responses(final_text: &str) -> Value {
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

/// In-process HTTP stub mirroring `rfl-openai-stub`'s recorded-response
/// contract (scope §A8 / §E1). Identical structure to the inlined stub
/// in `rfl_chat_demo_bar_send_mail.rs` — both predate the `rfl-openai-
/// stub` bin's hard-coded 5 s self-timeout window, which is shorter
/// than the `rfl init` + `rfl install` + `rfl chat` plugin-spawn-and-
/// validate sequence this test drives end-to-end (the in-process
/// listener has no self-timeout).
struct OpenAiStub {
    port: u16,
    shutdown: Arc<Mutex<bool>>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl OpenAiStub {
    fn start(responses: Value) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind stub listener");
        listener
            .set_nonblocking(true)
            .expect("set listener nonblocking");
        let port = listener.local_addr().expect("local_addr").port();
        let responses: Vec<Value> = responses.as_array().cloned().unwrap_or_default();
        assert!(!responses.is_empty(), "stub responses must be non-empty");
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
