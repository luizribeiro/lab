//! c13 — `entry` is required per scope §L2.

use rafaello_core::lock::Lock;

#[test]
fn missing_entry_field_rejected() {
    let toml = r#"
[plugin."local:foo@1.0.0"]
digest = "sha256:00"
manifest_digest = "sha256:00"
granted_at = "2026-01-15T08:30:00Z"
"#;
    let err = Lock::from_toml(toml).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("entry") || msg.contains("missing"),
        "got: {msg}"
    );
}
