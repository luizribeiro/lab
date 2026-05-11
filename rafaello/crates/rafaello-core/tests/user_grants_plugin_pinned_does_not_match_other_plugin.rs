use chrono::Utc;
use rafaello_core::lock::CanonicalId;
use rafaello_core::user_grants::{GrantMatcher, GrantSource, UserGrant, UserGrants};
use serde_json::json;

#[test]
fn plugin_pinned_does_not_match_other_plugin() {
    let plugin_a = CanonicalId::parse("github/acme:mailer@1.0.0").unwrap();
    let plugin_b = CanonicalId::parse("github/other:mailer@2.0.0").unwrap();
    let mut grants = UserGrants::new();
    let id = grants.add(UserGrant {
        tool: "send_mail".to_owned(),
        plugin: plugin_a.clone(),
        matcher: GrantMatcher::Any,
        added_at: Utc::now(),
        source: GrantSource::SlashCommand,
    });

    assert_eq!(grants.matches(&plugin_a, "send_mail", &json!({})), Some(id));
    assert_eq!(grants.matches(&plugin_b, "send_mail", &json!({})), None);
}
