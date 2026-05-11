//! c18 / scope §SL3: `/revoke <unknown_id>` publishes `command_result
//! {ok: false, kind: "revoke"}` and does not mutate `UserGrants`.

use serde_json::json;

mod common;
use common::slash_test_kit::{
    await_command_result, build_slash_rig, publish_slash, shutdown, subscribe_core_command_result,
    SlashRigOpts,
};

#[tokio::test]
async fn revoke_unknown_id_publishes_ok_false() {
    let rig = build_slash_rig(SlashRigOpts::default());
    let (mut rx, _sub) = subscribe_core_command_result(&rig.broker);
    let bogus = ulid::Ulid::new().to_string();
    publish_slash(
        &rig.broker,
        &rig.attach,
        json!({
            "command": "revoke",
            "args": {"grant_id": bogus},
        }),
    );

    let event = await_command_result(&mut rx).await;
    assert_eq!(event.payload["ok"], json!(false));
    assert_eq!(event.payload["kind"], json!("revoke"));
    assert!(event.payload["message"]
        .as_str()
        .unwrap_or("")
        .contains("unknown grant_id"));

    shutdown(rig).await;
}
