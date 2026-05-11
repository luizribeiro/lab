//! c18 / scope §SL3 step 1: a slash-command payload missing the
//! required `command`/`args` shape produces `command_result {ok:
//! false, kind: "unknown", message: "malformed payload"}` and an audit
//! row of kind `slash_unknown`.

use serde_json::json;

mod common;
use common::slash_test_kit::{
    await_command_result, build_slash_rig, publish_slash, shutdown, subscribe_core_command_result,
    SlashRigOpts,
};

#[tokio::test]
async fn malformed_payload_rejected() {
    let rig = build_slash_rig(SlashRigOpts::default());
    let (mut rx, _sub) = subscribe_core_command_result(&rig.broker);
    publish_slash(&rig.broker, &rig.attach, json!({"not_a_command": "oops"}));

    let event = await_command_result(&mut rx).await;
    assert_eq!(event.payload["ok"], json!(false));
    assert_eq!(event.payload["kind"], json!("unknown"));
    assert_eq!(event.payload["message"], json!("malformed payload"));

    let rows = rig.audit.rows();
    assert!(
        rows.iter().any(|(_, kind, _, _)| kind == "slash_unknown"),
        "expected a slash_unknown audit row, got {:?}",
        rows
    );

    shutdown(rig).await;
}
