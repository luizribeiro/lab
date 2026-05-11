use std::collections::BTreeMap;

use rafaello_core::user_grants::{GrantCompileError, UserGrants};
use serde_json::json;

#[test]
fn template_schema_mismatch_rejected() {
    let schema = json!({
        "type": "object",
        "properties": { "to": { "type": "string" } },
        "required": ["to"]
    });
    let mut user_args = BTreeMap::new();
    user_args.insert("to".to_owned(), json!(42));

    let err = UserGrants::compile_template("send_mail", user_args, Some(&schema))
        .expect_err("template should fail schema validation");

    match err {
        GrantCompileError::SchemaMismatch { diag } => {
            assert!(
                !diag.is_empty(),
                "expected non-empty jsonschema diagnostic, got empty"
            );
        }
    }
}
