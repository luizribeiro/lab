use std::path::PathBuf;

use anyhow::{bail, Result};

pub fn resolve_daemon_binary(binary_name: &str, env_override_var: &str) -> Result<PathBuf> {
    if let Some(path) = std::env::var_os(env_override_var) {
        let candidate = PathBuf::from(path);
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    if let Ok(current_exe) = std::env::current_exe() {
        let sibling = current_exe.with_file_name(binary_name);
        if sibling.exists() {
            return Ok(sibling);
        }
    }

    if let Some(in_path) = find_in_path(binary_name) {
        return Ok(in_path);
    }

    bail!(
        "unable to locate {binary_name} sidecar. Build/install it (e.g. `cargo build --bins`) and optionally set {env_override_var}"
    )
}

fn find_in_path(binary_name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(binary_name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::resolve_daemon_binary;
    use std::path::{Path, PathBuf};

    fn env_lock() -> &'static std::sync::Mutex<()> {
        crate::test_env_lock()
    }

    struct EnvVarGuard {
        key: &'static str,
        old: Option<std::ffi::OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &Path) -> Self {
            let old = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, old }
        }

        fn set_raw(key: &'static str, value: &str) -> Self {
            let old = std::env::var_os(key);
            std::env::set_var(key, value);
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

    fn make_temp_dir(prefix: &str) -> PathBuf {
        let unique = format!(
            "{prefix}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time should be after epoch")
                .as_nanos()
        );

        let dir = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&dir).expect("temp dir should be created");
        dir
    }

    #[test]
    fn env_override_path_wins_when_valid() {
        let _lock = env_lock().lock().expect("env lock should not be poisoned");
        let binary_name = "capsa-test-daemon-env-wins";
        let env_name = "CAPSA_TEST_DAEMON_ENV_WINS";

        let temp_dir = make_temp_dir("capsa-core-resolve-env");
        let override_path = temp_dir.join(binary_name);
        std::fs::write(&override_path, b"#!/bin/sh\n").expect("override file should be created");

        let _env = EnvVarGuard::set(env_name, &override_path);

        let resolved =
            resolve_daemon_binary(binary_name, env_name).expect("env override should resolve");

        assert_eq!(resolved, override_path);

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn invalid_env_override_falls_back_to_path() {
        let _lock = env_lock().lock().expect("env lock should not be poisoned");
        let binary_name = "capsa-test-daemon-path-fallback";
        let env_name = "CAPSA_TEST_DAEMON_PATH_FALLBACK";

        let temp_dir = make_temp_dir("capsa-core-resolve-path");
        let bin_path = temp_dir.join(binary_name);
        std::fs::write(&bin_path, b"#!/bin/sh\n").expect("path file should be created");

        let invalid_path = temp_dir.join("does-not-exist");
        let _override_env = EnvVarGuard::set(env_name, &invalid_path);
        let _path_env = EnvVarGuard::set_raw("PATH", temp_dir.to_string_lossy().as_ref());

        let resolved = resolve_daemon_binary(binary_name, env_name)
            .expect("resolver should fall back to PATH when env override is invalid");

        assert_eq!(resolved, bin_path);

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn returns_clear_error_when_binary_not_found() {
        let _lock = env_lock().lock().expect("env lock should not be poisoned");
        let binary_name = "capsa-test-daemon-missing";
        let env_name = "CAPSA_TEST_DAEMON_MISSING";

        let _override_env = EnvVarGuard::set_raw(env_name, "/definitely/missing/binary");
        let _path_env = EnvVarGuard::set_raw("PATH", "");

        let err = resolve_daemon_binary(binary_name, env_name)
            .expect_err("missing binary should return an error");
        let rendered = err.to_string();

        assert!(rendered.contains(binary_name));
        assert!(rendered.contains(env_name));
    }
}
