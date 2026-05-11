#![cfg(feature = "test-fixture")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::Duration;

#[test]
fn stub_responds_to_chat_completions_post_with_file_response() {
    let tmp = tempfile::tempdir().unwrap();
    let response_path = tmp.path().join("response.json");
    let canned = serde_json::json!([{
        "id": "chatcmpl-stub-1",
        "choices": [{
            "index": 0,
            "message": {"role": "assistant", "content": "hello from stub"},
            "finish_reason": "stop"
        }]
    }]);
    std::fs::write(&response_path, canned.to_string()).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_rfl-openai-stub"))
        .arg("--response")
        .arg(&response_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rfl-openai-stub");

    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut port_line = String::new();
    stdout.read_line(&mut port_line).unwrap();
    let port: u16 = port_line.trim().parse().expect("port on stdout");

    let body = serde_json::json!({
        "model": "gpt-stub",
        "messages": [{"role": "user", "content": "hi"}],
        "stream": false
    })
    .to_string();
    let mut sock = TcpStream::connect(("127.0.0.1", port)).unwrap();
    sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    write!(
        sock,
        "POST /v1/chat/completions HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    )
    .unwrap();
    let mut raw = String::new();
    sock.read_to_string(&mut raw).unwrap();

    let _ = child.kill();
    let _ = child.wait();

    assert!(raw.starts_with("HTTP/1.1 200"), "got: {raw}");
    let body = raw.split("\r\n\r\n").nth(1).unwrap_or("");
    let parsed: serde_json::Value = serde_json::from_str(body).expect("json body");
    assert_eq!(parsed["id"], "chatcmpl-stub-1");
    assert_eq!(
        parsed["choices"][0]["message"]["content"],
        "hello from stub"
    );
}
