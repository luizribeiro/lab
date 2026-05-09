//! c13 — Helper / helper_for fields are deferred (row 26); the
//! lock loader must refuse them via `deny_unknown_fields`.

use rafaello_core::lock::Lock;

const BASE: &str = r#"
[plugin."local:foo@1.0.0"]
entry = "main.js"
digest = "sha256:00"
manifest_digest = "sha256:00"
granted_at = "2026-01-15T08:30:00Z"
"#;

#[test]
fn bindings_helpers_field_rejected() {
    let toml = format!("{BASE}\n[plugin.\"local:foo@1.0.0\".bindings]\nhelpers = [\"helper.a\"]\n");
    let err = Lock::from_toml(&toml).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown field") || msg.contains("helpers"),
        "got: {msg}"
    );
}

#[test]
fn bindings_helper_for_field_rejected() {
    let toml =
        format!("{BASE}\n[plugin.\"local:foo@1.0.0\".bindings]\nhelper_for = \"some-tool\"\n");
    let err = Lock::from_toml(&toml).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown field") || msg.contains("helper_for"),
        "got: {msg}"
    );
}
