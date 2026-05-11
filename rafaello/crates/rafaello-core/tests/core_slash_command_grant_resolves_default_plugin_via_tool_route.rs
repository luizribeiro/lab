//! c18 / pi-4 B-2: with empty `session.tool_owner` and a single
//! mailcat plugin claiming `send-mail`, `/grant send-mail
//! to=alice@example.com` (no `plugin` arg) resolves the canonical via
//! `BrokerAcl::tool_route` and inserts the grant pinned to that
//! canonical.

use serde_json::json;

mod common;
use common::slash_test_kit::{
    await_command_result, build_slash_rig, mailcat_plugin_acl, publish_slash, shutdown,
    subscribe_core_command_result, SlashRigOpts, MAILCAT_CANONICAL,
};

#[tokio::test]
async fn grant_resolves_default_plugin_via_tool_route() {
    let rig = build_slash_rig(SlashRigOpts {
        plugins: vec![(
            common::slash_test_kit::cid(MAILCAT_CANONICAL),
            mailcat_plugin_acl(),
        )],
        tool_routes: vec![("send-mail", MAILCAT_CANONICAL)],
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
                "template": {"to": "alice@example.com"},
            },
        }),
    );

    let event = await_command_result(&mut rx).await;
    assert_eq!(event.payload["ok"], json!(true));

    let (plugin_str, tool) = {
        let grants = rig.user_grants.read();
        let (_, grant) = grants.list().into_iter().next().expect("one grant");
        (grant.plugin.to_string(), grant.tool.clone())
    };
    assert_eq!(plugin_str, MAILCAT_CANONICAL);
    assert_eq!(tool, "send-mail");

    shutdown(rig).await;
}
