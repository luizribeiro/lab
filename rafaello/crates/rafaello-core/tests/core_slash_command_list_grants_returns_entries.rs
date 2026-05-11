//! c18 / scope §SL3: `/grants list` enumerates the `UserGrants`
//! entries via the public `command_result.details.entries` projection.

use chrono::Utc;
use serde_json::json;

use rafaello_core::user_grants::{GrantMatcher, GrantSource, UserGrant};

mod common;
use common::slash_test_kit::{
    await_command_result, build_slash_rig, mailcat_plugin_acl, publish_slash, shutdown,
    subscribe_core_command_result, SlashRigOpts, MAILCAT_CANONICAL,
};

#[tokio::test]
async fn list_grants_returns_entries() {
    let rig = build_slash_rig(SlashRigOpts {
        plugins: vec![(
            common::slash_test_kit::cid(MAILCAT_CANONICAL),
            mailcat_plugin_acl(),
        )],
        ..Default::default()
    });
    let pre_id = rig.user_grants.lock().add(UserGrant {
        tool: "send-mail".to_string(),
        plugin: common::slash_test_kit::cid(MAILCAT_CANONICAL),
        matcher: GrantMatcher::Any,
        added_at: Utc::now(),
        source: GrantSource::SlashCommand,
    });

    let (mut rx, _sub) = subscribe_core_command_result(&rig.broker);
    publish_slash(
        &rig.broker,
        &rig.attach,
        json!({"command": "list_grants", "args": {}}),
    );

    let event = await_command_result(&mut rx).await;
    assert_eq!(event.payload["ok"], json!(true));
    assert_eq!(event.payload["kind"], json!("list_grants"));
    let entries = event.payload["details"]["entries"]
        .as_array()
        .expect("entries array");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["grant_id"], json!(pre_id.0.to_string()));
    assert_eq!(entries[0]["tool"], json!("send-mail"));
    assert_eq!(entries[0]["plugin"], json!(MAILCAT_CANONICAL));

    shutdown(rig).await;
}
