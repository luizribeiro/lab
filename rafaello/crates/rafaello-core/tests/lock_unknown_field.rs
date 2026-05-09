//! c13 — Lock-side `deny_unknown_fields` rejects extras.

use rafaello_core::lock::Lock;

#[test]
fn unknown_top_level_field_rejected() {
    let err = Lock::from_toml(r#"some_extra_table = 42"#).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown field") || msg.contains("some_extra_table"),
        "got: {msg}"
    );
}

#[test]
fn unknown_plugin_field_rejected() {
    let toml = r#"
[plugin."local:foo@1.0.0"]
entry = "main.js"
digest = "sha256:00"
manifest_digest = "sha256:00"
granted_at = "2026-01-15T08:30:00Z"
mystery = "field"
"#;
    let err = Lock::from_toml(toml).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown field") || msg.contains("mystery"),
        "got: {msg}"
    );
}

#[test]
fn unknown_session_field_rejected() {
    let toml = r#"
[session]
provider_active = "local:foo@1.0.0"
unknown_session_key = true
"#;
    let err = Lock::from_toml(toml).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("unknown field") || msg.contains("unknown_session_key"),
        "got: {msg}"
    );
}
