//! c31 acceptance: `SubprocessConnector` wires the spawned child's stdio
//! into a bidirectional `PeerHandle` after Group 2's API changes and Group
//! 3's call/with_service work. The parent uses `Client::peer().call` to
//! invoke a hand-rolled echo service in the child and `Client::peer().notify`
//! to push a notification the child observes via a side-channel log file.
//!
//! Per scope §P1 this is a verification commit — no public API changes in
//! `fittings-spawn` / `SubprocessConnector` itself.

#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use fittings::{Client, SubprocessConnector};
use serde_json::{json, Value};

fn unique_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("fittings-{name}-{}-{nanos}", std::process::id()))
}

fn write_executable_script(path: &Path, content: &str) {
    fs::write(path, content).expect("write fixture script");
    let mut perms = fs::metadata(path)
        .expect("read script metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).expect("set executable permissions");
}

#[tokio::test]
async fn subprocess_connector_round_trips_peer_call_and_notify() {
    let script_path = unique_path("spawn-peerhandle-rt");
    let notify_log = unique_path("spawn-peerhandle-rt-notify");
    let notify_log_escaped = notify_log.to_string_lossy().replace('"', "\\\"");

    // Hand-rolled echo service. Distinguishes requests from notifications by
    // the presence of `"id":"..."` (the client allocator emits string ids).
    // Requests get a JSON-RPC response on the same id; notifications are
    // appended to NOTIFY_LOG so the test can assert child reception.
    write_executable_script(
        &script_path,
        &format!(
            r#"#!/bin/sh
if [ "$FITTINGS" != "1" ]; then exit 90; fi
if [ "$1" != "serve" ]; then exit 91; fi
NOTIFY_LOG="{notify}"
while IFS= read -r line; do
  case "$line" in
    *'"id"'*)
      id=$(printf '%s' "$line" | sed -E 's/.*"id":"([^"]+)".*/\1/')
      printf '{{"jsonrpc":"2.0","id":"%s","result":{{"echoed":true}}}}\n' "$id"
      ;;
    *)
      printf '%s\n' "$line" >> "$NOTIFY_LOG"
      ;;
  esac
done
"#,
            notify = notify_log_escaped
        ),
    );

    let client = Client::connect(SubprocessConnector::new(&script_path))
        .await
        .expect("client should connect");

    let result = client
        .peer()
        .call("echo", json!({"hello": "world"}))
        .await
        .expect("peer.call should succeed");
    assert_eq!(result, json!({"echoed": true}));

    client
        .peer()
        .notify("greet", json!({"who": "world"}))
        .expect("peer.notify should enqueue");

    let mut received = None;
    for _ in 0..100 {
        if let Ok(text) = fs::read_to_string(&notify_log) {
            if !text.is_empty() {
                received = Some(text);
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    drop(client);

    let received = received.expect("notification should reach child within timeout");
    let line = received
        .lines()
        .next()
        .expect("at least one notification line");
    let value: Value = serde_json::from_str(line).expect("notification frame is JSON");
    assert!(
        value.get("id").is_none(),
        "notification frame must omit id: {value}",
    );
    assert_eq!(value["jsonrpc"], "2.0");
    assert_eq!(value["method"], "greet");
    assert_eq!(value["params"], json!({"who": "world"}));

    let _ = fs::remove_file(&script_path);
    let _ = fs::remove_file(&notify_log);
}
