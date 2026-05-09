#![cfg(feature = "test-fixture")]

use std::process::Command;

#[test]
fn fixture_binary_unknown_mode_exits_64() {
    let path = env!("CARGO_BIN_EXE_rfl-bus-fixture");
    let status = Command::new(path)
        .env("RFL_FIXTURE_MODE", "bogus")
        .status()
        .expect("spawn rfl-bus-fixture");
    assert_eq!(
        status.code(),
        Some(64),
        "expected exit 64, got {:?}",
        status
    );
}
