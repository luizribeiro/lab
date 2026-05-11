//! Scope §OP5 + pi-3 B-1 + pi-4 B-1: the combined m5a fixture lock
//! pins `session.provider_active` to `builtin:openai@0.0.0` and
//! leaves `session.tool_owner` empty (no claim conflict between the
//! one mailcat `send-mail` declarer and the one readfile
//! `read-file` declarer).

use std::path::PathBuf;

use rafaello_core::lock::Lock;

#[test]
fn session_pins_provider_active() {
    let lock_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("m5a-locks")
        .join("rafaello.lock");
    let raw = std::fs::read_to_string(&lock_path).expect("read combined fixture lock");
    let lock = Lock::from_toml(&raw).expect("parse combined fixture lock");

    assert_eq!(
        lock.session.provider_active.as_deref(),
        Some("builtin:openai@0.0.0"),
    );
    assert!(
        lock.session.tool_owner.is_empty(),
        "tool_owner must be empty under the no-conflict invariant: {:?}",
        lock.session.tool_owner
    );
}
