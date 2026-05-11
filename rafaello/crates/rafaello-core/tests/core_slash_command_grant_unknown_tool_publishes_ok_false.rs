//! c18 / pi-4 B-2: `/grant nonexistent` against a lock where no plugin
//! claims the tool publishes `command_result {ok: false, message: "no
//! plugin provides tool 'nonexistent'"}` and writes a `slash_unknown`
//! audit row.

use serde_json::json;

mod common;
use common::slash_test_kit::{
    await_command_result, build_slash_rig, publish_slash, shutdown, subscribe_core_command_result,
    SlashRigOpts,
};

#[tokio::test]
async fn grant_unknown_tool_publishes_ok_false() {
    let rig = build_slash_rig(SlashRigOpts::default());
    let (mut rx, _sub) = subscribe_core_command_result(&rig.broker);
    publish_slash(
        &rig.broker,
        &rig.attach,
        json!({
            "command": "grant",
            "args": {
                "tool": "nonexistent",
                "template": {},
            },
        }),
    );

    let event = await_command_result(&mut rx).await;
    assert_eq!(event.payload["ok"], json!(false));
    assert_eq!(event.payload["kind"], json!("grant"));
    assert_eq!(
        event.payload["message"],
        json!("no plugin provides tool 'nonexistent'")
    );

    let rows = rig.audit.rows();
    assert!(
        rows.iter().any(|(_, kind, _, _)| kind == "slash_unknown"),
        "expected a slash_unknown audit row, got {:?}",
        rows
    );
    assert_eq!(rig.user_grants.lock().list().len(), 0);

    shutdown(rig).await;
}
