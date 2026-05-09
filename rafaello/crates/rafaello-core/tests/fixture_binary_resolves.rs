#![cfg(feature = "test-fixture")]

use std::process::Command;

#[test]
fn fixture_binary_resolves_and_scaffold_only_exits_zero() {
    let path = env!("CARGO_BIN_EXE_rfl-bus-fixture");
    let status = Command::new(path)
        .env("RFL_FIXTURE_MODE", "scaffold_only")
        .status()
        .expect("spawn rfl-bus-fixture");
    assert!(status.success(), "expected exit 0, got {:?}", status);
}
