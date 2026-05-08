//! c13 — Lock loader applies the M11 `SafePath` rule to `.entry`
//! (scope §L2): rejects `..`, leading `/`, backslash, etc.

use rafaello_core::lock::Lock;

fn lock_with_entry(entry: &str) -> String {
    format!(
        r#"
[plugin."local:foo@1.0.0"]
entry = "{entry}"
digest = "sha256:00"
manifest_digest = "sha256:00"
granted_at = "2026-01-15T08:30:00Z"
"#
    )
}

#[test]
fn parent_dir_segment_rejected() {
    let err = Lock::from_toml(&lock_with_entry("../escape.js")).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("parent-dir") || msg.contains(".."),
        "got: {msg}"
    );
}

#[test]
fn leading_slash_rejected() {
    let err = Lock::from_toml(&lock_with_entry("/etc/passwd")).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("leading slash") || msg.contains("safepath"),
        "got: {msg}"
    );
}

#[test]
fn backslash_rejected() {
    let err = Lock::from_toml(&lock_with_entry("bin\\\\evil.js")).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("backslash") || msg.contains("safepath"),
        "got: {msg}"
    );
}
