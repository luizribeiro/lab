use chrono::Utc;
use rafaello_core::lock::CanonicalId;
use rafaello_core::user_grants::{GrantMatcher, GrantSource, UserGrant, UserGrants};
use serde_json::json;

#[test]
fn structural_matcher_missing_key_does_not_match() {
    let plugin = CanonicalId::parse("github/acme:mailer@1.0.0").unwrap();
    let mut grants = UserGrants::new();
    grants.add(UserGrant {
        tool: "send_mail".to_owned(),
        plugin: plugin.clone(),
        matcher: GrantMatcher::Structural {
            template: json!({"to": "a@b.c", "subject": "hello"}),
        },
        added_at: Utc::now(),
        source: GrantSource::SlashCommand,
    });

    assert_eq!(
        grants.matches(&plugin, "send_mail", &json!({"to": "a@b.c"})),
        None
    );
}
