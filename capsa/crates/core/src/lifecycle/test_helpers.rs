//! Shared test helpers for `lifecycle/` integration tests. Used by
//! both `child::tests` and `orchestrate::tests` to avoid
//! duplicating the env-var guard, temp-path generator, and binary
//! lookup helpers.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub(crate) fn env_lock() -> &'static Mutex<()> {
    crate::test_env_lock()
}

pub(crate) struct EnvVarGuard {
    key: &'static str,
    old: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    pub(crate) fn set(key: &'static str, value: &str) -> Self {
        let old = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, old }
    }

    pub(crate) fn set_path(key: &'static str, value: &Path) -> Self {
        let old = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, old }
    }

    pub(crate) fn unset(key: &'static str) -> Self {
        let old = std::env::var_os(key);
        std::env::remove_var(key);
        Self { key, old }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(old) = self.old.take() {
            std::env::set_var(self.key, old);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

pub(crate) fn unique_temp_path(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time should be after epoch")
            .as_nanos()
    ))
}

pub(crate) fn find_binary_on_path(name: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path_var = std::env::var_os("PATH").expect("PATH should be set");
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(name);
        let Ok(meta) = std::fs::metadata(&candidate) else {
            continue;
        };
        if meta.is_file() && (meta.permissions().mode() & 0o111) != 0 {
            return candidate;
        }
    }
    panic!("binary `{name}` should be on PATH for tests");
}

/// Absolute path to the `capsa-fake-netd` helper binary built as part
/// of this crate. Cargo builds the `[[bin]]` target into
/// `target/<profile>/capsa-fake-netd`; `env::current_exe()` lets us
/// reach it from any test binary.
#[cfg(target_os = "linux")]
pub(crate) fn fake_netd_path() -> PathBuf {
    let test_exe = std::env::current_exe().expect("current_exe should succeed");
    test_exe
        .parent()
        .and_then(|p| p.parent())
        .expect("test binary should live under target/<profile>/deps")
        .join("capsa-fake-netd")
}
