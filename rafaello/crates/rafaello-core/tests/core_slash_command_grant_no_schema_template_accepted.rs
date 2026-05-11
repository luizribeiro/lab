//! c18 / scope §SL3 + §UG: when the tool declares no `grant_match`
//! schema, a `/grant` template with arbitrary string args is accepted
//! and a `UserGrant` is inserted.

use serde_json::json;

mod common;
use common::slash_test_kit::{
    await_command_result, build_slash_rig, mailcat_plugin_acl, publish_slash, shutdown,
    subscribe_core_command_result, SlashRigOpts, MAILCAT_CANONICAL,
};

#[tokio::test]
async fn grant_no_schema_template_accepted() {
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
                "template": {"any": "value"},
            },
        }),
    );

    let event = await_command_result(&mut rx).await;
    assert_eq!(event.payload["ok"], json!(true));
    assert_eq!(event.payload["kind"], json!("grant"));
    assert_eq!(rig.user_grants.lock().list().len(), 1);

    shutdown(rig).await;
}
