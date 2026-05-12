//! Regression for m6 retro round-3 pi-2 §B1.
//!
//! When the caller of `SandboxedCommand` invokes `env_clear()` —
//! the canonical pattern for plugin supervisors that build child
//! env from a declared manifest policy rather than inheriting the
//! parent — the `CARGO_BIN_EXE_syd-pty` env that lockin installed
//! for syd's PTY-helper discovery must survive into the syd child
//! environment. Without `apply_sandbox_internal_env` running just
//! before `spawn`, env_clear strips it and syd fires its
//! `setup_pty` "syd-pty spawn error" path on the canonical
//! cold-start `rfl chat` flow. Asserts the re-application via
//! fake-syd's env-record sentinel file.

#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};

use lockin::SandboxBuilder;

#[test]
fn fake_syd_records_cargo_bin_exe_env_after_env_clear() {
    let work = tempfile::tempdir().expect("tempdir");
    let pty_fixture = work.path().join("syd-pty-fixture");
    std::fs::write(&pty_fixture, b"").expect("write syd-pty fixture");
    let pty_fixture = pty_fixture
        .canonicalize()
        .expect("canonicalize syd-pty fixture");
    let record_path = work.path().join("fake-syd-record.json");

    let fake_syd = PathBuf::from(env!("CARGO_BIN_EXE_fake-syd"));

    let mut cmd = SandboxBuilder::new()
        .syd_path(&fake_syd)
        .syd_pty_path(&pty_fixture)
        .command(Path::new("/bin/true"))
        .expect("build sandbox command");

    // Wipe the sandbox-internal env that lockin set on the syd
    // command, mirroring `rafaello-core/src/supervisor.rs::spawn`'s
    // `cmd.env_clear()` call. Pre-fix this drops
    // `CARGO_BIN_EXE_syd-pty` entirely; post-fix
    // `apply_sandbox_internal_env` re-applies it at spawn time.
    cmd.env_clear();
    // The record-path env still has to be passed back in because
    // env_clear wiped it too — fake-syd reads it from its own env.
    cmd.env("RFL_FAKE_SYD_RECORD_PATH", &record_path);

    let mut child = cmd.spawn().expect("spawn fake-syd");
    let status = child.wait().expect("wait fake-syd");
    assert!(status.success(), "fake-syd exited non-zero: {status:?}");

    let blob = std::fs::read_to_string(&record_path).expect("read record");
    let expected = format!("CARGO_BIN_EXE_syd-pty={}", pty_fixture.display());
    assert!(
        blob.contains(&expected),
        "record missing {expected:?} after env_clear; \
         this is the m6 retro round-3 pi-2 §B1 regression: {blob}"
    );
}
