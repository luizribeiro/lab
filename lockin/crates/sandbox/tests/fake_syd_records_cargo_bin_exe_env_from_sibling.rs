//! With no `.syd_pty_path(...)` and no `CARGO_BIN_EXE_syd-pty` in the
//! process env, resolution falls through to the sibling-discovery arm:
//! the `syd-pty` placed next to `syd` is the one fake-syd records on
//! the child's environment.

#![cfg(target_os = "linux")]

use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use lockin::SandboxBuilder;

struct EnvRestore {
    key: &'static str,
    saved: Option<OsString>,
}

impl EnvRestore {
    fn clear(key: &'static str) -> Self {
        let saved = std::env::var_os(key);
        std::env::remove_var(key);
        Self { key, saved }
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        match self.saved.take() {
            Some(v) => std::env::set_var(self.key, v),
            None => std::env::remove_var(self.key),
        }
    }
}

#[test]
fn fake_syd_records_cargo_bin_exe_env_from_sibling() {
    let _pty_clear = EnvRestore::clear("CARGO_BIN_EXE_syd-pty");

    let work = tempfile::tempdir().expect("tempdir");
    let dir = work.path().canonicalize().expect("canonicalize tempdir");

    let fake_syd_src = PathBuf::from(env!("CARGO_BIN_EXE_fake-syd"));
    let fake_syd_dst = dir.join("fake-syd");
    std::fs::copy(&fake_syd_src, &fake_syd_dst).expect("copy fake-syd");
    let mut perms = std::fs::metadata(&fake_syd_dst)
        .expect("stat fake-syd copy")
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&fake_syd_dst, perms).expect("chmod fake-syd copy");

    let pty_sibling = dir.join("syd-pty");
    std::fs::write(&pty_sibling, b"").expect("write sibling syd-pty");

    let record_path = dir.join("fake-syd-record.json");

    let mut cmd = SandboxBuilder::new()
        .syd_path(&fake_syd_dst)
        .command(Path::new("/bin/true"))
        .expect("build sandbox command");
    cmd.env("RFL_FAKE_SYD_RECORD_PATH", &record_path);

    let mut child = cmd.spawn().expect("spawn fake-syd");
    let status = child.wait().expect("wait fake-syd");
    assert!(status.success(), "fake-syd exited non-zero: {status:?}");

    let blob = std::fs::read_to_string(&record_path).expect("read record");
    let expected = format!("CARGO_BIN_EXE_syd-pty={}", pty_sibling.display());
    assert!(
        blob.contains(&expected),
        "record missing sibling-resolved {expected:?}: {blob}"
    );
}
