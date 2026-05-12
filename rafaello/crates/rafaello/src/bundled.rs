//! Resolves bundled-plugin source trees and runtime binaries (scope §A0/§A1/§A2).
//!
//! Three resolvers, all with the same three-arm shape: env override / release
//! layout / dev-workspace fallback. Public fns call `std::env::current_exe()`
//! and delegate to a `_from_exe_parent` seam so unit tests can target the seam
//! with a `tempfile::tempdir()`-rooted parent rather than mutating the real
//! `<workspace>/target/...` tree.

use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum BundledError {
    #[error("bundled plugin '{name}' not found (set RFL_BUNDLED_PLUGINS_DIR or install share/)")]
    NotFound { name: String },
    #[error("io error resolving rfl binary path: {0}")]
    Io(#[from] std::io::Error),
}

pub struct BundledPluginNames {
    /// Stable logical identifier. Dev-fallback walks up to
    /// `<workspace>/crates/rafaello-<dev_crate>/`; the
    /// `RFL_BUNDLED_PLUGINS_DIR` env-arm joins under this name; and the
    /// `RFL_BUNDLED_BIN_<NAME_UPPER>` env override munges from it.
    pub dev_crate: &'static str,
    /// Release-arm plugin directory under
    /// `$out/share/rafaello/plugins/<release_dir>/`.
    pub release_dir: &'static str,
    /// Runtime binary file name. Release path:
    /// `<release_dir>/bin/<runtime_bin>`. Dev path:
    /// `<workspace>/target/<profile>/<runtime_bin>`.
    pub runtime_bin: &'static str,
}

pub const OPENAI_NAMES: BundledPluginNames = BundledPluginNames {
    dev_crate: "openai",
    release_dir: "rfl-openai",
    runtime_bin: "rfl-openai",
};

pub fn resolve_plugin_dir(name: &str) -> Result<PathBuf, BundledError> {
    let exe = std::env::current_exe()?;
    let parent = exe.parent().ok_or_else(|| BundledError::NotFound {
        name: name.to_owned(),
    })?;
    resolve_plugin_dir_from_exe_parent(parent, name)
}

pub fn resolve_plugin_dir_for_bundled(names: &BundledPluginNames) -> Result<PathBuf, BundledError> {
    let exe = std::env::current_exe()?;
    let parent = exe.parent().ok_or_else(|| BundledError::NotFound {
        name: names.dev_crate.to_owned(),
    })?;
    resolve_plugin_dir_for_bundled_from_exe_parent(parent, names)
}

pub fn resolve_runtime_binary(names: &BundledPluginNames) -> Result<PathBuf, BundledError> {
    let exe = std::env::current_exe()?;
    let parent = exe.parent().ok_or_else(|| BundledError::NotFound {
        name: names.runtime_bin.to_owned(),
    })?;
    resolve_runtime_binary_from_exe_parent(parent, names)
}

fn resolve_plugin_dir_from_exe_parent(parent: &Path, name: &str) -> Result<PathBuf, BundledError> {
    if let Some(dir) = std::env::var_os("RFL_BUNDLED_PLUGINS_DIR") {
        let candidate = PathBuf::from(dir).join(name);
        if candidate.is_dir() {
            return Ok(candidate);
        }
    }

    let release = release_plugin_dir(parent, name);
    if release.is_dir() {
        return Ok(release);
    }

    let crate_name = format!("rafaello-{name}");
    for ancestor in parent.ancestors() {
        let candidate = ancestor.join("crates").join(&crate_name);
        if candidate.is_dir() {
            return Ok(candidate);
        }
    }

    Err(BundledError::NotFound {
        name: name.to_owned(),
    })
}

fn resolve_plugin_dir_for_bundled_from_exe_parent(
    parent: &Path,
    names: &BundledPluginNames,
) -> Result<PathBuf, BundledError> {
    if let Some(dir) = std::env::var_os("RFL_BUNDLED_PLUGINS_DIR") {
        let candidate = PathBuf::from(dir).join(names.dev_crate);
        if candidate.is_dir() {
            return Ok(candidate);
        }
    }

    let release = release_plugin_dir(parent, names.release_dir);
    if release.is_dir() {
        return Ok(release);
    }

    if let Some(ws) = find_workspace_root(parent) {
        let dev = ws
            .join("crates")
            .join(format!("rafaello-{}", names.dev_crate));
        if dev.is_dir() {
            return Ok(dev);
        }
    }

    Err(BundledError::NotFound {
        name: format!(
            "bundled source tree for '{}' not found (tried RFL_BUNDLED_PLUGINS_DIR/{}, {}, workspace crates/rafaello-{})",
            names.dev_crate,
            names.dev_crate,
            release.display(),
            names.dev_crate,
        ),
    })
}

fn resolve_runtime_binary_from_exe_parent(
    parent: &Path,
    names: &BundledPluginNames,
) -> Result<PathBuf, BundledError> {
    let env_var = env_var_name_for(names.dev_crate);
    if let Some(val) = std::env::var_os(&env_var) {
        let p = PathBuf::from(val);
        if p.is_absolute() && is_user_executable_regular_file(&p) {
            return Ok(p);
        }
    }

    let release = release_plugin_dir(parent, names.release_dir)
        .join("bin")
        .join(names.runtime_bin);
    if release.is_file() {
        return Ok(release);
    }

    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let dev = find_workspace_root(parent)
        .map(|ws| ws.join("target").join(profile).join(names.runtime_bin));
    if let Some(ref d) = dev {
        if d.is_file() {
            return Ok(d.clone());
        }
    }

    let dev_display = match dev {
        Some(p) => p.display().to_string(),
        None => format!("workspace target/{profile}/{}", names.runtime_bin),
    };

    Err(BundledError::NotFound {
        name: format!(
            "no {} runtime binary discoverable (tried env {}, {}, {})",
            names.runtime_bin,
            env_var,
            release.display(),
            dev_display,
        ),
    })
}

fn release_plugin_dir(parent: &Path, dir_name: &str) -> PathBuf {
    parent
        .join("..")
        .join("share")
        .join("rafaello")
        .join("plugins")
        .join(dir_name)
}

fn env_var_name_for(dev_crate: &str) -> String {
    format!(
        "RFL_BUNDLED_BIN_{}",
        dev_crate.replace('-', "_").to_uppercase()
    )
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    for ancestor in start.ancestors() {
        let cargo = ancestor.join("Cargo.toml");
        if cargo.is_file() {
            if let Ok(contents) = std::fs::read_to_string(&cargo) {
                if contents.contains("[workspace]") {
                    return Some(ancestor.to_path_buf());
                }
            }
        }
    }
    None
}

#[cfg(unix)]
fn is_user_executable_regular_file(p: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match std::fs::metadata(p) {
        Ok(m) => m.is_file() && (m.permissions().mode() & 0o100 != 0),
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn is_user_executable_regular_file(p: &Path) -> bool {
    p.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    const ENV_PLUGINS_DIR: &str = "RFL_BUNDLED_PLUGINS_DIR";
    const ENV_BIN_OPENAI: &str = "RFL_BUNDLED_BIN_OPENAI";
    const ENV_BIN_FOO_BAR: &str = "RFL_BUNDLED_BIN_FOO_BAR";

    struct EnvGuard {
        keys: Vec<&'static str>,
    }

    impl EnvGuard {
        fn new() -> Self {
            Self { keys: Vec::new() }
        }

        fn set(&mut self, key: &'static str, value: &Path) {
            std::env::set_var(key, value);
            self.track(key);
        }

        fn unset(&mut self, key: &'static str) {
            std::env::remove_var(key);
            self.track(key);
        }

        fn track(&mut self, key: &'static str) {
            if !self.keys.contains(&key) {
                self.keys.push(key);
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for k in &self.keys {
                std::env::remove_var(k);
            }
        }
    }

    fn make_exe_parent(tmp: &TempDir) -> PathBuf {
        let parent = tmp.path().join("bin");
        fs::create_dir_all(&parent).unwrap();
        parent
    }

    fn make_release_plugin_dir(tmp: &TempDir, release_dir: &str) -> PathBuf {
        let p = tmp
            .path()
            .join("share")
            .join("rafaello")
            .join("plugins")
            .join(release_dir);
        fs::create_dir_all(&p).unwrap();
        p
    }

    fn make_synthetic_workspace(root: &Path) {
        fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
    }

    #[cfg(unix)]
    fn set_mode(p: &Path, mode: u32) {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(p).unwrap().permissions();
        perms.set_mode(mode);
        fs::set_permissions(p, perms).unwrap();
    }

    fn canon(p: &Path) -> PathBuf {
        p.canonicalize().unwrap()
    }

    #[test]
    #[serial(bundled_env)]
    fn resolve_plugin_dir_release_arm_uses_input_name_verbatim() {
        let mut g = EnvGuard::new();
        g.unset(ENV_PLUGINS_DIR);
        let tmp = TempDir::new().unwrap();
        let parent = make_exe_parent(&tmp);
        let target = make_release_plugin_dir(&tmp, "rfl-mailcat");
        let unexpected = tmp
            .path()
            .join("share")
            .join("rafaello")
            .join("plugins")
            .join("rfl-rfl-mailcat");
        assert!(!unexpected.exists());

        let got = resolve_plugin_dir_from_exe_parent(&parent, "rfl-mailcat").unwrap();
        assert_eq!(canon(&got), canon(&target));
    }

    #[test]
    #[serial(bundled_env)]
    fn resolve_plugin_dir_for_bundled_env_arm_hit() {
        let mut g = EnvGuard::new();
        let tmp = TempDir::new().unwrap();
        let parent = make_exe_parent(&tmp);
        let env_root = tmp.path().join("env-root");
        let target = env_root.join("openai");
        fs::create_dir_all(&target).unwrap();
        fs::write(target.join("rafaello.toml"), "").unwrap();
        g.set(ENV_PLUGINS_DIR, &env_root);

        let got = resolve_plugin_dir_for_bundled_from_exe_parent(&parent, &OPENAI_NAMES).unwrap();
        assert_eq!(canon(&got), canon(&target));
    }

    #[test]
    #[serial(bundled_env)]
    fn resolve_plugin_dir_for_bundled_release_arm_hit() {
        let mut g = EnvGuard::new();
        g.unset(ENV_PLUGINS_DIR);
        let tmp = TempDir::new().unwrap();
        let parent = make_exe_parent(&tmp);
        let target = make_release_plugin_dir(&tmp, "rfl-openai");

        let got = resolve_plugin_dir_for_bundled_from_exe_parent(&parent, &OPENAI_NAMES).unwrap();
        assert_eq!(canon(&got), canon(&target));
    }

    #[test]
    #[serial(bundled_env)]
    fn resolve_plugin_dir_for_bundled_dev_fallback_hit() {
        let mut g = EnvGuard::new();
        g.unset(ENV_PLUGINS_DIR);
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().join("ws");
        fs::create_dir_all(&ws).unwrap();
        make_synthetic_workspace(&ws);
        let target = ws.join("crates").join("rafaello-openai");
        fs::create_dir_all(&target).unwrap();
        let parent = ws.join("target").join("debug");
        fs::create_dir_all(&parent).unwrap();

        let got = resolve_plugin_dir_for_bundled_from_exe_parent(&parent, &OPENAI_NAMES).unwrap();
        assert_eq!(canon(&got), canon(&target));
    }

    #[test]
    #[serial(bundled_env)]
    #[cfg(unix)]
    fn resolve_runtime_binary_env_override_hit() {
        let mut g = EnvGuard::new();
        g.unset(ENV_PLUGINS_DIR);
        let tmp = TempDir::new().unwrap();
        let parent = make_exe_parent(&tmp);
        let sentinel = tmp.path().join("sentinel");
        fs::write(&sentinel, b"#!/bin/sh\n").unwrap();
        set_mode(&sentinel, 0o755);
        g.set(ENV_BIN_OPENAI, &sentinel);

        let got = resolve_runtime_binary_from_exe_parent(&parent, &OPENAI_NAMES).unwrap();
        assert_eq!(canon(&got), canon(&sentinel));

        set_mode(&sentinel, 0o644);
        let err = resolve_runtime_binary_from_exe_parent(&parent, &OPENAI_NAMES).unwrap_err();
        assert!(matches!(err, BundledError::NotFound { .. }));
    }

    #[test]
    #[serial(bundled_env)]
    #[cfg(unix)]
    fn resolve_runtime_binary_release_arm_hit() {
        let mut g = EnvGuard::new();
        g.unset(ENV_PLUGINS_DIR);
        g.unset(ENV_BIN_OPENAI);
        let tmp = TempDir::new().unwrap();
        let parent = make_exe_parent(&tmp);
        let plugin = make_release_plugin_dir(&tmp, "rfl-openai");
        let bin_dir = plugin.join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let target = bin_dir.join("rfl-openai");
        fs::write(&target, b"#!/bin/sh\n").unwrap();
        set_mode(&target, 0o755);

        let got = resolve_runtime_binary_from_exe_parent(&parent, &OPENAI_NAMES).unwrap();
        assert_eq!(canon(&got), canon(&target));
    }

    #[test]
    #[serial(bundled_env)]
    #[cfg(unix)]
    fn resolve_runtime_binary_dev_fallback_hit() {
        let mut g = EnvGuard::new();
        g.unset(ENV_PLUGINS_DIR);
        g.unset(ENV_BIN_OPENAI);
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().join("ws");
        fs::create_dir_all(&ws).unwrap();
        make_synthetic_workspace(&ws);
        let target_dir = ws.join("target").join("debug");
        fs::create_dir_all(&target_dir).unwrap();
        let target = target_dir.join("rfl-openai");
        fs::write(&target, b"#!/bin/sh\n").unwrap();
        set_mode(&target, 0o755);

        let got = resolve_runtime_binary_from_exe_parent(&target_dir, &OPENAI_NAMES).unwrap();
        assert_eq!(canon(&got), canon(&target));
    }

    #[test]
    #[serial(bundled_env)]
    fn resolve_runtime_binary_not_found_lists_all_arms() {
        let mut g = EnvGuard::new();
        g.unset(ENV_PLUGINS_DIR);
        g.unset(ENV_BIN_OPENAI);
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().join("ws");
        fs::create_dir_all(&ws).unwrap();
        make_synthetic_workspace(&ws);
        let target_dir = ws.join("target").join("debug");
        fs::create_dir_all(&target_dir).unwrap();

        let err = resolve_runtime_binary_from_exe_parent(&target_dir, &OPENAI_NAMES).unwrap_err();
        let msg = format!("{err}");
        assert!(msg.contains("RFL_BUNDLED_BIN_OPENAI"), "msg={msg}");
        assert!(
            msg.contains("share/rafaello/plugins/rfl-openai/bin/rfl-openai"),
            "msg={msg}"
        );
        assert!(msg.contains("target/debug/rfl-openai"), "msg={msg}");
    }

    #[test]
    #[serial(bundled_env)]
    #[cfg(unix)]
    fn resolve_runtime_binary_env_var_name_munge() {
        let mut g = EnvGuard::new();
        g.unset(ENV_PLUGINS_DIR);
        let tmp = TempDir::new().unwrap();
        let parent = make_exe_parent(&tmp);
        let sentinel = tmp.path().join("foo-bar-bin");
        fs::write(&sentinel, b"#!/bin/sh\n").unwrap();
        set_mode(&sentinel, 0o755);
        g.set(ENV_BIN_FOO_BAR, &sentinel);

        let names = BundledPluginNames {
            dev_crate: "foo-bar",
            release_dir: "rfl-foo-bar",
            runtime_bin: "rfl-foo-bar",
        };
        let got = resolve_runtime_binary_from_exe_parent(&parent, &names).unwrap();
        assert_eq!(canon(&got), canon(&sentinel));
    }
}
