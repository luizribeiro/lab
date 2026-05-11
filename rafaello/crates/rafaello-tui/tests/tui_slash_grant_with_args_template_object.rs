//! c17 / scope §SL5: `/grant <tool> k1=v1 k2=v2` produces args.template
//! as a JSON object (BTreeMap<String, Value>) with each k=v as a string.

mod common;

use std::time::Duration;

use serde_json::json;

use common::{spawn_tui, wait_for_method, RecordingService, SpawnOpts};

#[tokio::test(flavor = "multi_thread")]
async fn slash_grant_with_args_template_object() {
    let (recorder, mut events) = RecordingService::new();
    let mut h = spawn_tui(
        SpawnOpts {
            test_mode: true,
            max_lifetime: Some(5),
            ready_delay_ms: None,
            test_message: Some("/grant tool_a path=/etc cmd=ls".to_string()),
            test_confirm_answer: None,
            test_confirm_delay_ms: None,
            test_grant_before_message: None,
        },
        recorder,
    );

    let _ready = wait_for_method(&mut events, "frontend.ready", Duration::from_secs(3)).await;
    let publish = wait_for_method(&mut events, "bus.publish", Duration::from_secs(3)).await;

    assert_eq!(
        publish.params.get("topic").and_then(|v| v.as_str()),
        Some("frontend.tui.slash_command")
    );
    let args = publish
        .params
        .get("payload")
        .and_then(|p| p.get("args"))
        .expect("args");
    assert_eq!(args.get("tool").and_then(|v| v.as_str()), Some("tool_a"));
    assert_eq!(
        args.get("template").expect("template"),
        &json!({ "path": "/etc", "cmd": "ls" })
    );

    drop(h.parent_peer);
    let _ = h.child.kill().await;
}
