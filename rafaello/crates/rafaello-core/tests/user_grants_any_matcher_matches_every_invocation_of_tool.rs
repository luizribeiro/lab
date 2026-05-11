use chrono::Utc;
use rafaello_core::lock::CanonicalId;
use rafaello_core::user_grants::{GrantMatcher, GrantSource, UserGrant, UserGrants};
use serde_json::json;

#[test]
fn any_matcher_matches_every_invocation_of_tool() {
    let plugin = CanonicalId::parse("github/acme:mailer@1.0.0").unwrap();
    let mut grants = UserGrants::new();
    let id = grants.add(UserGrant {
        tool: "send_mail".to_owned(),
        plugin: plugin.clone(),
        matcher: GrantMatcher::Any,
        added_at: Utc::now(),
        source: GrantSource::SlashCommand,
    });

    assert_eq!(grants.matches(&plugin, "send_mail", &json!({})), Some(id));
    assert_eq!(
        grants.matches(&plugin, "send_mail", &json!({"to": "a@b.c"})),
        Some(id)
    );
    assert_eq!(
        grants.matches(&plugin, "send_mail", &json!({"to": "x", "subject": "y"})),
        Some(id)
    );
    assert_eq!(grants.matches(&plugin, "other_tool", &json!({})), None);
}
