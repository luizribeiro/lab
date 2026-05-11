use std::collections::BTreeMap;

use rafaello_core::user_grants::{GrantMatcher, UserGrants};
use serde_json::json;

#[test]
fn template_no_schema_declared_accepted() {
    let mut user_args = BTreeMap::new();
    user_args.insert("to".to_owned(), json!("x"));

    let matcher = UserGrants::compile_template("send_mail", user_args, None)
        .expect("template accepted with no schema declared");

    match matcher {
        GrantMatcher::Structural { template } => {
            assert_eq!(template, json!({"to": "x"}));
        }
        other => panic!("expected Structural matcher, got {:?}", other),
    }
}
