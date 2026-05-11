#![cfg(feature = "test-fixture")]

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn stub_self_timeout_exits_within_lifetime() {
    let tmp = tempfile::tempdir().unwrap();
    let response_path = tmp.path().join("response.json");
    let canned = serde_json::json!([{
        "id": "x",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "ok"}, "finish_reason": "stop"}]
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
    assert!(
        port_line.trim().parse::<u16>().is_ok(),
        "expected port: {port_line:?}"
    );

    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait().expect("try_wait") {
            break s;
        }
        if start.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            panic!("stub did not exit within 10s (5s self-timeout + slack)");
        }
        std::thread::sleep(Duration::from_millis(100));
    };

    assert!(start.elapsed() < Duration::from_secs(10));
    assert!(
        start.elapsed() >= Duration::from_secs(4),
        "exited too early: {:?}",
        start.elapsed()
    );
    assert!(status.success(), "expected clean exit, got {status:?}");
}
