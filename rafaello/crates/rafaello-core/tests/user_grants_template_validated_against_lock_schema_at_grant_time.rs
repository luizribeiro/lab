use std::collections::BTreeMap;

use rafaello_core::user_grants::{GrantMatcher, UserGrants};
use serde_json::json;

#[test]
fn template_validated_against_lock_schema_at_grant_time() {
    let schema = json!({
        "type": "object",
        "properties": { "to": { "type": "string" } },
        "required": ["to"]
    });
    let mut user_args = BTreeMap::new();
    user_args.insert("to".to_owned(), json!("alice@example.com"));

    let matcher = UserGrants::compile_template("send_mail", user_args, Some(&schema))
        .expect("template validates against schema");

    match matcher {
        GrantMatcher::Structural { template } => {
            assert_eq!(template, json!({"to": "alice@example.com"}));
        }
        other => panic!("expected Structural matcher, got {:?}", other),
    }
}
