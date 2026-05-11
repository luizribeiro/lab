//! c18 / scope §SL3 + §UG: a `/grant` with a template that fails the
//! tool's `grant_match` schema publishes `command_result {ok: false}`
//! and inserts no grant.

use std::collections::BTreeMap;

use serde_json::json;

mod common;
use common::slash_test_kit::{
    await_command_result, build_slash_rig, mailcat_plugin_acl, publish_slash, shutdown,
    subscribe_core_command_result, SlashRigOpts, MAILCAT_CANONICAL,
};

#[tokio::test]
async fn grant_template_schema_mismatch_publishes_ok_false() {
    let mut schemas = BTreeMap::new();
    schemas.insert(
        "send-mail".to_string(),
        json!({
            "type": "object",
            "properties": { "to": { "type": "string", "format": "email" } },
            "required": ["to", "subject"]
        }),
    );
    let rig = build_slash_rig(SlashRigOpts {
        plugins: vec![(
            common::slash_test_kit::cid(MAILCAT_CANONICAL),
            mailcat_plugin_acl(),
        )],
        schemas,
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

    let event = await_command_result(&mut rx).await;
    assert_eq!(event.payload["ok"], json!(false));
    assert_eq!(event.payload["kind"], json!("grant"));
    assert!(
        event.payload["message"]
            .as_str()
            .unwrap_or("")
            .contains("schema mismatch"),
        "message should mention schema mismatch: {:?}",
        event.payload["message"]
    );
    assert_eq!(rig.user_grants.read().list().len(), 0);

    shutdown(rig).await;
}
