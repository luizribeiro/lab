//! Integration tests for the scripted-turns mode of `rfl-openai-stub`
//! (scope §E2). Each test spawns the live binary, reads the assigned
//! port from stdout, drives the HTTP surface directly, and asserts on
//! the response body or on the deterministic exit-plus-stderr signal.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream as StdTcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

fn write_script(dir: &Path, turns: &[(&str, &str, &str)]) -> PathBuf {
    let path = dir.join("script.toml");
    let mut out = String::new();
    for (kind, value, response_json) in turns {
        out.push_str("[[turn]]\n");
        out.push_str(&format!("{kind} = \"{value}\"\n"));
        out.push_str(&format!("response = '''{response_json}'''\n\n"));
    }
    std::fs::write(&path, out).expect("write script");
    path
}

fn spawn_with_script(script: &Path) -> (Child, u16) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rfl-openai-stub"))
        .env("RFL_OPENAI_STUB_SCRIPTED_TURNS", script)
        .env_remove("RFL_OPENAI_STUB_RESPONSE")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rfl-openai-stub");
    let mut stdout = BufReader::new(child.stdout.take().expect("child stdout"));
    let mut port_line = String::new();
    stdout
        .read_line(&mut port_line)
        .expect("read port from stub stdout");
    let port: u16 = port_line
        .trim()
        .parse()
        .unwrap_or_else(|e| panic!("parse port {port_line:?}: {e}"));
    (child, port)
}

fn post(port: u16, body: &str) -> std::io::Result<String> {
    let mut sock = StdTcpStream::connect(("127.0.0.1", port))?;
    sock.set_read_timeout(Some(Duration::from_secs(2)))?;
    write!(
        sock,
        "POST /v1/chat/completions HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )?;
    let mut raw = String::new();
    sock.read_to_string(&mut raw)?;
    Ok(raw)
}

fn parse_ok_body(raw: &str) -> serde_json::Value {
    assert!(raw.starts_with("HTTP/1.1 200"), "expected 200, got: {raw}");
    let body = raw.split("\r\n\r\n").nth(1).unwrap_or("");
    serde_json::from_str(body).expect("response body is JSON")
}

fn drain_stderr(child: &mut Child) -> String {
    let mut s = String::new();
    if let Some(mut err) = child.stderr.take() {
        err.read_to_string(&mut s).ok();
    }
    s
}

#[test]
fn two_turn_happy_path_send_mail_flow() {
    let tmp = tempfile::tempdir().unwrap();
    let turn1 = serde_json::json!({
        "id": "chatcmpl-stub-1",
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [{
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "send-mail",
                        "arguments": "{\"to\":\"alice@example.com\",\"body\":\"hello\"}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }]
    })
    .to_string();
    let turn2 = serde_json::json!({
        "id": "chatcmpl-stub-2",
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "Done. Mail dispatched to alice@example.com."},
            "finish_reason": "stop"
        }]
    })
    .to_string();
    let script = write_script(
        tmp.path(),
        &[
            ("match_last_user_message", "send", &turn1),
            ("match_last_tool_call_function", "send-mail", &turn2),
        ],
    );
    let (mut child, port) = spawn_with_script(&script);

    let req1 = serde_json::json!({
        "model": "gpt-stub",
        "messages": [{"role": "user", "content": "send alice@example.com a hello note"}],
        "stream": false
    })
    .to_string();
    let raw1 = post(port, &req1).expect("first POST");
    let v1 = parse_ok_body(&raw1);
    assert_eq!(
        v1["choices"][0]["message"]["tool_calls"][0]["function"]["name"], "send-mail",
        "first turn must surface the send-mail tool call: {v1}"
    );

    let req2 = serde_json::json!({
        "model": "gpt-stub",
        "messages": [
            {"role": "user", "content": "send alice@example.com a hello note"},
            {"role": "assistant", "content": null, "tool_calls": [{
                "id": "call_1",
                "type": "function",
                "function": {"name": "send-mail", "arguments": "{}"}
            }]},
            {"role": "tool", "tool_call_id": "call_1", "content": "ok"}
        ],
        "stream": false
    })
    .to_string();
    let raw2 = post(port, &req2).expect("second POST");
    let v2 = parse_ok_body(&raw2);
    let content = v2["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or_default();
    assert!(
        content.contains("Done"),
        "second turn content must contain Done: {content:?}"
    );

    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn exhaustion_exits_deterministically() {
    let tmp = tempfile::tempdir().unwrap();
    let only = serde_json::json!({
        "id": "chatcmpl-only",
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "only turn"},
            "finish_reason": "stop"
        }]
    })
    .to_string();
    let script = write_script(tmp.path(), &[("match_last_user_message", "ping", &only)]);
    let (mut child, port) = spawn_with_script(&script);

    let req = serde_json::json!({
        "model": "gpt-stub",
        "messages": [{"role": "user", "content": "ping me please"}],
        "stream": false
    })
    .to_string();
    let raw = post(port, &req).expect("first POST consumes only turn");
    let _ = parse_ok_body(&raw);

    // Second POST drives the cursor past the end; the connection task
    // calls process::exit(1) mid-handler, so the client sees either a
    // connection-reset error or a zero-byte response. Both are tolerated.
    let _ = post(port, &req);

    let status = child.wait().expect("wait child");
    assert!(
        !status.success(),
        "stub must exit non-zero after exhaustion, got {status:?}"
    );
    let stderr = drain_stderr(&mut child);
    assert!(
        stderr.contains("scripted turns exhausted"),
        "stderr must carry the exhaustion signal: {stderr:?}"
    );
}

#[test]
fn mutual_exclusion_with_response_env() {
    let tmp = tempfile::tempdir().unwrap();
    let response_file = tmp.path().join("response.json");
    std::fs::write(
        &response_file,
        serde_json::json!([{
            "id": "x",
            "choices": [{
                "index": 0,
                "message": {"role": "assistant", "content": "x"},
                "finish_reason": "stop"
            }]
        }])
        .to_string(),
    )
    .unwrap();
    let script_file = write_script(
        tmp.path(),
        &[(
            "match_last_user_message",
            "anything",
            "{\"id\":\"x\",\"choices\":[]}",
        )],
    );

    let mut child = Command::new(env!("CARGO_BIN_EXE_rfl-openai-stub"))
        .env("RFL_OPENAI_STUB_SCRIPTED_TURNS", &script_file)
        .env("RFL_OPENAI_STUB_RESPONSE", &response_file)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rfl-openai-stub");

    let status = child.wait().expect("wait child");
    assert!(
        !status.success(),
        "stub must refuse to start with both selectors set, got {status:?}"
    );

    let mut stdout_str = String::new();
    if let Some(mut out) = child.stdout.take() {
        out.read_to_string(&mut stdout_str).ok();
    }
    assert!(
        stdout_str.trim().is_empty(),
        "no port should reach stdout when startup is rejected, got: {stdout_str:?}"
    );

    let stderr = drain_stderr(&mut child);
    assert!(
        stderr.contains("mutually exclusive"),
        "stderr must explain that scripted and response envs are mutually exclusive: {stderr:?}"
    );
}

#[test]
fn unmatched_predicate_exits_deterministically() {
    let tmp = tempfile::tempdir().unwrap();
    let resp = serde_json::json!({
        "id": "chatcmpl-unreachable",
        "object": "chat.completion",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "unreachable"},
            "finish_reason": "stop"
        }]
    })
    .to_string();
    let script = write_script(
        tmp.path(),
        &[("match_last_user_message", "needle-that-is-absent", &resp)],
    );
    let (mut child, port) = spawn_with_script(&script);

    let req = serde_json::json!({
        "model": "gpt-stub",
        "messages": [{"role": "user", "content": "this content does not contain the predicate"}],
        "stream": false
    })
    .to_string();
    let _ = post(port, &req);

    let status = child.wait().expect("wait child");
    assert!(
        !status.success(),
        "stub must exit non-zero on first non-matching turn, got {status:?}"
    );
    let stderr = drain_stderr(&mut child);
    // The live stub funnels both cursor-exhaustion and predicate-miss into
    // a single deterministic exit path emitting "scripted turns exhausted".
    assert!(
        stderr.contains("scripted turns exhausted"),
        "stderr must carry the deterministic exhaustion signal on predicate miss: {stderr:?}"
    );
}
