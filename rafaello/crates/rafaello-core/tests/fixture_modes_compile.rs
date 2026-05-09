#![cfg(feature = "test-fixture")]

use std::process::Command;

const MISSING_FD_MODES: &[&str] = &[
    "respond_peer_call",
    "publish_one",
    "publish_with_taint",
    "publish_full_params",
    "publish_bad_namespace",
    "publish_bad_grammar",
    "publish_outside_grant",
    "publish_bad_in_reply_to_missing",
    "publish_bad_in_reply_to_empty",
    "publish_bad_in_reply_to_multiple",
    "call_core_then_exit",
    "observer",
];

#[test]
fn fixture_modes_dispatch_recognised() {
    let path = env!("CARGO_BIN_EXE_rfl-bus-fixture");

    for mode in MISSING_FD_MODES {
        let status = Command::new(path)
            .env_clear()
            .env("RFL_FIXTURE_MODE", mode)
            .status()
            .expect("spawn rfl-bus-fixture");
        assert_eq!(
            status.code(),
            Some(3),
            "mode {} expected exit 3 (mode recognised, RFL_BUS_FD missing), got {:?}",
            mode,
            status,
        );
    }

    let status = Command::new(path)
        .env_clear()
        .env("RFL_FIXTURE_MODE", "scaffold_only")
        .status()
        .expect("spawn rfl-bus-fixture");
    assert_eq!(status.code(), Some(0), "scaffold_only should exit 0");

    let status = Command::new(path)
        .env_clear()
        .env("RFL_FIXTURE_MODE", "definitely-not-a-mode")
        .status()
        .expect("spawn rfl-bus-fixture");
    assert_eq!(
        status.code(),
        Some(64),
        "unknown mode should exit 64, got {:?}",
        status
    );
}
