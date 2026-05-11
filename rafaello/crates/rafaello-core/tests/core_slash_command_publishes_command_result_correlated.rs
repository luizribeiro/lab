//! c18 / scope §SL0: the `command_result` envelope `in_reply_to`
//! carries exactly the slash command's envelope `request_id` (single
//! entry, c11 cardinality).

use serde_json::json;

mod common;
use common::slash_test_kit::{
    await_command_result, build_slash_rig, mailcat_plugin_acl, publish_slash, shutdown,
    subscribe_core_command_result, SlashRigOpts, MAILCAT_CANONICAL,
};

#[tokio::test]
async fn publishes_command_result_correlated() {
    let rig = build_slash_rig(SlashRigOpts {
        plugins: vec![(
            common::slash_test_kit::cid(MAILCAT_CANONICAL),
            mailcat_plugin_acl(),
        )],
        ..Default::default()
    });
    let (mut rx, _sub) = subscribe_core_command_result(&rig.broker);
    let slash_id = publish_slash(
        &rig.broker,
        &rig.attach,
        json!({
            "command": "grant",
            "args": {
                "tool": "send-mail",
                "plugin": MAILCAT_CANONICAL,
                "template": {},
            },
        }),
    );

    let event = await_command_result(&mut rx).await;
    let in_reply_to = event
        .in_reply_to
        .as_ref()
        .expect("envelope in_reply_to set");
    assert_eq!(in_reply_to.len(), 1);
    assert_eq!(&in_reply_to[0], &slash_id);
    assert!(event.request_id.is_some(), "core mints a fresh request_id");
    assert!(
        event.payload.get("request_id").is_none(),
        "payload carries no request_id (§SL0 implication 1)"
    );

    shutdown(rig).await;
}
