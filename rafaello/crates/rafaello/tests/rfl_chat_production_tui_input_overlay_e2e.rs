//! cK5 — tmux-driven end-to-end integration test for the production
//! TUI input field + confirm overlay key handlers wired in cK1..cK4.
//!
//! Unlike the c27 sibling
//! (`rfl_chat_demo_bar_init_install_chat_confirm_persist.rs`), this
//! test deliberately does **not** set `RFL_TUI_TEST_MODE`,
//! `RFL_TUI_TEST_MESSAGE`, `RFL_TUI_TEST_CONFIRM_ANSWER`, or
//! `RFL_TUI_TEST_CONFIRM_ANSWERS` (the m5b row 56 hook). The whole
//! point of Phase K is to prove the production-mode wiring — keystrokes
//! flow PTY → crossterm `EventStream` → `handle_terminal_event` →
//! `publish_submitted_line` / `publish_confirm_answer` →
//! `bus.publish` → supervisor — without the test hooks short-circuiting
//! that path. The user's input line and the overlay answer are keyed
//! in via `tmux send-keys`; the rendered overlay copy is asserted via
//! `tmux capture-pane`.
//!
//! Authenticity bar matches c27 fix `de8e187`: real captured behavior,
//! not authored. Linux-only (same discipline as `syd-pty`-dependent
//! tests per scope §"Acceptance summary").

#![cfg(target_os = "linux")]

mod common;

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use common::install_test_kit::copy_in_tree_to_bundled_dir;
use common::workspace_bin_path::workspace_bin;
use rafaello_core::session::SessionStore;
use rusqlite::Connection;
use serde_json::{json, Value};
use serial_test::serial;

const ALICE: &str = "alice@example.com";

#[test]
#[serial(rfl_chat)]
fn rfl_chat_production_tui_input_overlay_e2e() {
    if Command::new("tmux").arg("-V").output().is_err() {
        eprintln!("tmux not available; skipping cK5 production-TUI e2e test");
        return;
    }

    let _ = workspace_bin("rfl");
    let _ = workspace_bin("rfl-tui");
    let real_openai = workspace_bin("rfl-openai");
    let real_mailcat = workspace_bin("rfl-mailcat");

    let project = tempfile::tempdir().unwrap();
    let project_root = project.path();

    let stub = OpenAiStub::start(stub_responses("Email sent to alice."));
    let endpoint = format!("http://127.0.0.1:{}/v1", stub.port);

    let bundled = tempfile::tempdir().unwrap();
    write_openai_bundled_tree(&bundled.path().join("openai"), &endpoint, &real_openai);
    let mailcat_dir =
        copy_in_tree_to_bundled_dir(bundled.path(), "rfl-mailcat", "rafaello-mailcat");
    let mailcat_bin = mailcat_dir.join("bin").join("rfl-mailcat");
    fs::copy(&real_mailcat, &mailcat_bin).expect("copy real rfl-mailcat into bundled tree");
    fs::set_permissions(&mailcat_bin, fs::Permissions::from_mode(0o755)).unwrap();
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
        "rfl install failed: stderr={}",
        String::from_utf8_lossy(&install.stderr)
    );

    // tmux does NOT pass arbitrary parent env vars to a `new-session`'s
    // child command, so bake the env into a wrapper shell script. The
    // wrapper also redirects rfl chat's stderr to a log file we can
    // include in panic messages for diagnosis.
    let chat_wrapper = bundled.path().join("rfl-chat-wrapper.sh");
    let log_path = bundled.path().join("rfl-chat.stderr");
    fs::write(
        &chat_wrapper,
        format!(
            "#!/bin/sh\n\
             export RFL_TUI_PATH='{tui}'\n\
             export LITELLM_API_KEY='sk-cK5-prod-tui'\n\
             export TERM='xterm-256color'\n\
             exec '{rfl}' chat --project-root '{root}' 2>'{log}'\n",
            tui = workspace_bin("rfl-tui").display(),
            rfl = rfl.display(),
            root = project_root.display(),
            log = log_path.display(),
        ),
    )
    .unwrap();
    fs::set_permissions(&chat_wrapper, fs::Permissions::from_mode(0o755)).unwrap();

    let session = format!("rfl-cK5-{}", std::process::id());
    let new = Command::new("tmux")
        .args(["new-session", "-d", "-s", &session, "-x", "100", "-y", "30"])
        .arg(&chat_wrapper)
        .output()
        .expect("spawn tmux new-session");
    assert!(
        new.status.success(),
        "tmux new-session failed: stderr={}",
        String::from_utf8_lossy(&new.stderr)
    );

    let kill_guard = TmuxSessionGuard {
        session: session.clone(),
    };

    // Wait for `rfl chat` to spawn the TUI frontend before we start
    // keying input. The production ui_loop only paints on the first
    // event, so we can't poll the pane for a prompt — instead we poll
    // `rfl chat`'s stderr for the supervisor's frontend-ready sentinel.
    poll_for_stderr_line(
        &log_path,
        "frontend-ready-observed",
        Duration::from_secs(30),
    );

    tmux_send(&session, "Please email alice@example.com a status update");
    // tmux's `Enter` keyname sends LF (0x0a) under our terminfo, which
    // crossterm parses as Ctrl-J (`Char('j')`) rather than `Enter`.
    // Send carriage return (C-m) explicitly to match `KeyCode::Enter`.
    tmux_key(&session, "C-m");

    let confirm_substrings: &[&str] = &[" confirm ", "send-mail", "sinks: mail", "s remaining"];
    let pane_after_prompt =
        poll_pane_for_all(&session, confirm_substrings, Duration::from_secs(120)).unwrap_or_else(
            |pane| {
                let stderr = fs::read_to_string(&log_path).unwrap_or_default();
                panic!(
                    "confirm overlay never rendered all expected substrings.\n\
                 pane:\n{pane}\n\
                 rfl chat stderr:\n{stderr}"
                )
            },
        );
    for s in confirm_substrings {
        assert!(
            pane_after_prompt.contains(s),
            "pane missing {s:?}; pane:\n{pane_after_prompt}"
        );
    }

    tmux_send(&session, "a");

    let state_dir = project_root.join(".rafaello").join("state");
    let sqlite_path = state_dir.join("session.sqlite");

    let deadline = Instant::now() + Duration::from_secs(60);
    let (last_entries, last_audit) = loop {
        let entries_kinds = SessionStore::open(&state_dir)
            .ok()
            .and_then(|s| s.load_entries().ok())
            .map(|rows| {
                rows.iter()
                    .map(|s| s.entry.kind.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let audit_kinds = read_audit_kinds(&sqlite_path);
        if entries_kinds.iter().any(|k| k == "tool_result")
            && audit_kinds.iter().any(|k| k == "confirm_allowed")
        {
            break (entries_kinds, audit_kinds);
        }
        if Instant::now() >= deadline {
            let pane = tmux_capture(&session);
            let stderr = fs::read_to_string(&log_path).unwrap_or_default();
            panic!(
                "post-allow DB rows never landed within 20s.\n\
                 entries={entries_kinds:?}\naudit={audit_kinds:?}\n\
                 pane:\n{pane}\nrfl chat stderr:\n{stderr}"
            );
        }
        thread::sleep(Duration::from_millis(200));
    };

    assert!(
        last_entries.iter().any(|k| k == "tool_call"),
        "entries missing tool_call; got {last_entries:?}"
    );
    assert!(
        last_entries.iter().any(|k| k == "tool_result"),
        "entries missing tool_result; got {last_entries:?}"
    );
    assert!(
        last_audit.iter().any(|k| k == "confirm_request"),
        "audit missing confirm_request; got {last_audit:?}"
    );
    assert!(
        last_audit.iter().any(|k| k == "confirm_allowed"),
        "audit missing confirm_allowed; got {last_audit:?}"
    );

    let pane_post_allow = tmux_capture(&session);
    assert!(
        !pane_post_allow.contains(" confirm ") || pane_post_allow.contains("Email sent"),
        "overlay should clear (or be replaced by assistant follow-up) after `a`; pane:\n{pane_post_allow}"
    );

    tmux_key(&session, "q");
    let exit_deadline = Instant::now() + Duration::from_secs(10);
    while session_alive(&session) {
        if Instant::now() >= exit_deadline {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    let alive_after_q = session_alive(&session);
    drop(kill_guard);
    drop(stub);

    assert!(
        !alive_after_q,
        "tmux session still alive after `q`; rfl chat did not exit cleanly. stderr:\n{}",
        fs::read_to_string(&log_path).unwrap_or_default()
    );

    let user_text_row = SessionStore::open(&state_dir)
        .expect("reopen session store")
        .load_entries()
        .expect("load entries")
        .into_iter()
        .find(|s| {
            s.entry.kind == "text"
                && s.entry
                    .payload
                    .get("text")
                    .and_then(|v| v.as_str())
                    .map(|t| t.contains(ALICE))
                    .unwrap_or(false)
        });
    assert!(
        user_text_row.is_some(),
        "user-typed text entry not persisted — keystroke path didn't reach the session store"
    );
}

struct TmuxSessionGuard {
    session: String,
}

impl Drop for TmuxSessionGuard {
    fn drop(&mut self) {
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", &self.session])
            .output();
    }
}

fn tmux_send(session: &str, text: &str) {
    let status = Command::new("tmux")
        .args(["send-keys", "-l", "-t", session, text])
        .status()
        .expect("tmux send-keys -l");
    assert!(status.success(), "tmux send-keys -l failed");
}

fn tmux_key(session: &str, key: &str) {
    let status = Command::new("tmux")
        .args(["send-keys", "-t", session, key])
        .status()
        .expect("tmux send-keys key");
    assert!(status.success(), "tmux send-keys {key} failed");
}

fn tmux_capture(session: &str) -> String {
    let out = Command::new("tmux")
        .args(["capture-pane", "-t", session, "-p"])
        .output()
        .expect("tmux capture-pane");
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn poll_for_stderr_line(log_path: &Path, needle: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        if fs::read_to_string(log_path)
            .unwrap_or_default()
            .contains(needle)
        {
            return;
        }
        if Instant::now() >= deadline {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn poll_pane_for_all(session: &str, needles: &[&str], timeout: Duration) -> Result<String, String> {
    let deadline = Instant::now() + timeout;
    loop {
        let last = tmux_capture(session);
        if needles.iter().all(|n| last.contains(n)) {
            return Ok(last);
        }
        if Instant::now() >= deadline {
            return Err(last);
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn session_alive(session: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", session])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn read_audit_kinds(db: &Path) -> Vec<String> {
    let conn = match Connection::open(db) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let mut stmt = match conn.prepare("SELECT kind FROM audit_events ORDER BY seq") {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    stmt.query_map([], |r| r.get::<_, String>(0))
        .map(|it| it.filter_map(Result::ok).collect())
        .unwrap_or_default()
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

fn exec_dirs_toml_array() -> String {
    let mut dirs: Vec<String> = Vec::new();
    if let Some(val) = std::env::var_os("LOCKIN_TEST_EXEC_DIRS") {
        for d in std::env::split_paths(&val) {
            if d.is_absolute() {
                dirs.push(d.to_string_lossy().into_owned());
            }
        }
    }
    let temp_root = std::env::temp_dir().to_string_lossy().into_owned();
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

/// In-process HTTP stub mirroring `rfl-openai-stub`'s scripted-turns
/// contract. The on-disk `rfl-openai-stub` bin's hard-coded 5 s
/// self-timeout is shorter than `rfl init` + `rfl install` + `rfl chat`
/// + tmux drive can take in CI; an in-process listener has no timeout.
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
        let join = thread::spawn(move || {
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
                        thread::sleep(Duration::from_millis(20));
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
