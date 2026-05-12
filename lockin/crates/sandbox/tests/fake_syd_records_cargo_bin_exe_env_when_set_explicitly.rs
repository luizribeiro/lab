//! With `.syd_pty_path(...)` set explicitly on the builder, fake-syd
//! (run as the syd binary) receives `CARGO_BIN_EXE_syd-pty=<that path>`
//! on its environment when the sandbox launches.

#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};

use lockin::SandboxBuilder;

#[test]
fn fake_syd_records_cargo_bin_exe_env_when_set_explicitly() {
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
    cmd.env("RFL_FAKE_SYD_RECORD_PATH", &record_path);

    let mut child = cmd.spawn().expect("spawn fake-syd");
    let status = child.wait().expect("wait fake-syd");
    assert!(status.success(), "fake-syd exited non-zero: {status:?}");

    let blob = std::fs::read_to_string(&record_path).expect("read record");
    let expected = format!("CARGO_BIN_EXE_syd-pty={}", pty_fixture.display());
    assert!(
        blob.contains(&expected),
        "record missing {expected:?}: {blob}"
    );
}
