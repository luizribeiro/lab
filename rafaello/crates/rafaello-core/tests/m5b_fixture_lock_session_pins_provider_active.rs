//! Scope §TF3 + pi-4 B-1 — the combined m5b fixture lock pins
//! `session.provider_active` to `builtin:openai@0.0.0` and leaves
//! `session.tool_owner` empty. Each tool (`web-fetch`, `send-mail`,
//! `read-file`) has exactly one claimant — live `validate::lock`
//! rejects redundant entries with `ToolOwnerRedundant`.

use std::path::PathBuf;

use rafaello_core::lock::Lock;

#[test]
fn session_pins_provider_active() {
    let lock_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("m5b-locks")
        .join("rafaello.lock");
    let raw = std::fs::read_to_string(&lock_path).expect("read m5b fixture lock");
    let lock = Lock::from_toml(&raw).expect("parse m5b fixture lock");

    assert_eq!(
        lock.session.provider_active.as_deref(),
        Some("builtin:openai@0.0.0"),
    );
    assert!(
        lock.session.tool_owner.is_empty(),
        "tool_owner must be empty under the single-claimant invariant: {:?}",
        lock.session.tool_owner
    );
}
