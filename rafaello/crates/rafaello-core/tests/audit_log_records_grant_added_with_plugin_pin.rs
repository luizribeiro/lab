//! c18 / scope §SL3 + §UG1: the `grant_added` audit row carries the
//! canonical plugin pin (distinct from a tool-name-only audit row);
//! this anchors the §UG1 "plugin-pinned, not name-pinned" assertion.

use serde_json::json;

mod common;
use common::slash_test_kit::{
    await_command_result, build_slash_rig, mailcat_plugin_acl, publish_slash, shutdown,
    subscribe_core_command_result, SlashRigOpts, MAILCAT_CANONICAL,
};

#[tokio::test]
async fn audit_log_records_grant_added_with_plugin_pin() {
    let rig = build_slash_rig(SlashRigOpts {
        plugins: vec![(
            common::slash_test_kit::cid(MAILCAT_CANONICAL),
            mailcat_plugin_acl(),
        )],
        ..Default::default()
    });
    let (mut rx, _sub) = subscribe_core_command_result(&rig.broker);
    publish_slash(
        &rig.broker,
        &rig.attach,
        json!({
            "command": "grant",
            "args": {
                "tool": "send-mail",
                "plugin": MAILCAT_CANONICAL,
                "template": {"to": "alice@example.com"},
            },
        }),
    );
    let _ = await_command_result(&mut rx).await;

    let rows = rig.audit.rows();
    let grant_row = rows
        .iter()
        .find(|(_, kind, _, _)| kind == "grant_added")
        .expect("a grant_added audit row was recorded");
    assert_eq!(grant_row.3["plugin"], json!(MAILCAT_CANONICAL));
    assert_eq!(grant_row.3["tool"], json!("send-mail"));
    assert_eq!(grant_row.3["source"], json!("SlashCommand"));

    shutdown(rig).await;
}
