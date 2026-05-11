use chrono::Utc;
use rafaello_core::lock::CanonicalId;
use rafaello_core::user_grants::{GrantMatcher, GrantSource, UserGrant, UserGrants};
use serde_json::json;

#[test]
fn structural_matcher_extra_args_still_matches() {
    let plugin = CanonicalId::parse("github/acme:mailer@1.0.0").unwrap();
    let mut grants = UserGrants::new();
    let id = grants.add(UserGrant {
        tool: "send_mail".to_owned(),
        plugin: plugin.clone(),
        matcher: GrantMatcher::Structural {
            template: json!({"to": "a@b.c"}),
        },
        added_at: Utc::now(),
        source: GrantSource::SlashCommand,
    });

    assert_eq!(
        grants.matches(
            &plugin,
            "send_mail",
            &json!({"to": "a@b.c", "subject": "hi", "cc": "x@y.z"})
        ),
        Some(id)
    );
}
