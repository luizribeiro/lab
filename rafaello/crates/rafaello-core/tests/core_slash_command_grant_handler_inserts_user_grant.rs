//! c18 / scope §SL3: a well-formed `/grant` with an explicit plugin
//! pins inserts a `UserGrant` and publishes `command_result {ok: true,
//! kind: "grant"}` correlated to the slash command.

use serde_json::json;

mod common;
use common::slash_test_kit::{
    await_command_result, build_slash_rig, mailcat_plugin_acl, publish_slash, shutdown,
    subscribe_core_command_result, SlashRigOpts, MAILCAT_CANONICAL,
};

#[tokio::test]
async fn grant_handler_inserts_user_grant() {
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
                "template": {"to": "alice@example.com"},
            },
        }),
    );

    let event = await_command_result(&mut rx).await;
    assert_eq!(event.payload["ok"], json!(true));
    assert_eq!(event.payload["kind"], json!("grant"));
    assert_eq!(event.in_reply_to.as_deref(), Some(&[slash_id][..]));

    assert_eq!(rig.user_grants.lock().list().len(), 1);

    shutdown(rig).await;
}
