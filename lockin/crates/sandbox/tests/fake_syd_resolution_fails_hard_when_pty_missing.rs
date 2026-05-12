//! With no `.syd_pty_path(...)`, no `CARGO_BIN_EXE_syd-pty` env, no
//! sibling next to the resolved `syd`, and no `syd-pty` in PATH,
//! `.command(...)` must surface a hard error mentioning syd-pty —
//! there is no silent `pty:off` fallback.

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
    fn set(key: &'static str, val: &str) -> Self {
        let saved = std::env::var_os(key);
        std::env::set_var(key, val);
        Self { key, saved }
    }

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
fn fake_syd_resolution_fails_hard_when_pty_missing() {
    let _pty_clear = EnvRestore::clear("CARGO_BIN_EXE_syd-pty");

    let empty_path_dir = tempfile::tempdir().expect("empty PATH dir");
    let _path_scope = EnvRestore::set(
        "PATH",
        empty_path_dir.path().to_str().expect("tempdir is utf-8"),
    );

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

    let err = SandboxBuilder::new()
        .syd_path(&fake_syd_dst)
        .command(Path::new("/bin/true"))
        .err()
        .expect("expected hard error when syd-pty is unresolvable");
    let msg = format!("{err}");
    assert!(
        msg.contains("Linux sandbox requires syd-pty"),
        "expected syd-pty failure message, got: {msg}"
    );
}
