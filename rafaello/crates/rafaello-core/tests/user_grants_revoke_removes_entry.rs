use chrono::Utc;
use rafaello_core::lock::CanonicalId;
use rafaello_core::user_grants::{GrantMatcher, GrantSource, UserGrant, UserGrants};
use serde_json::json;

#[test]
fn revoke_removes_entry() {
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
    grants.revoke(id).expect("revoke should succeed");
    assert_eq!(grants.matches(&plugin, "send_mail", &json!({})), None);
    assert!(grants.list().is_empty());
}
