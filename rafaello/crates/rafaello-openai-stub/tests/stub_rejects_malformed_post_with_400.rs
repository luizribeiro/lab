#![cfg(feature = "test-fixture")]

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::Duration;

#[test]
fn stub_rejects_malformed_post_with_400_and_logs_to_stderr() {
    let tmp = tempfile::tempdir().unwrap();
    let response_path = tmp.path().join("response.json");
    let canned = serde_json::json!([{
        "id": "x",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "ok"}, "finish_reason": "stop"}]
    }]);
    std::fs::write(&response_path, canned.to_string()).unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_rfl-openai-stub"))
        .env("RFL_OPENAI_STUB_RESPONSE", &response_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn rfl-openai-stub");

    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut port_line = String::new();
    stdout.read_line(&mut port_line).unwrap();
    let port: u16 = port_line.trim().parse().expect("port on stdout");

    let body = "{ this is not valid json";
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
    let output = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(raw.starts_with("HTTP/1.1 400"), "got: {raw}");
    assert!(
        stderr.contains("malformed request body"),
        "expected malformed-body stderr signal, got: {stderr}"
    );
}
