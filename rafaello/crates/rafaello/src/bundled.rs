//! Resolves the on-disk source tree of a bundled plugin (scope §A2).
//!
//! Resolution order:
//! 1. `RFL_BUNDLED_PLUGINS_DIR` env var (used by tests + dev invocations).
//! 2. Release layout: `<rfl-exe-parent>/../share/rafaello/plugins/<name>/`.
//! 3. Dev fallback: walk up from the `rfl` binary looking for a workspace
//!    root containing `crates/rafaello-<name>/`.

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum BundledError {
    #[error("bundled plugin '{name}' not found (set RFL_BUNDLED_PLUGINS_DIR or install share/)")]
    NotFound { name: String },
    #[error("io error resolving rfl binary path: {0}")]
    Io(#[from] std::io::Error),
}

pub fn resolve_plugin_dir(name: &str) -> Result<PathBuf, BundledError> {
    if let Some(dir) = std::env::var_os("RFL_BUNDLED_PLUGINS_DIR") {
        let candidate = PathBuf::from(dir).join(name);
        if candidate.is_dir() {
            return Ok(candidate);
        }
    }

    let exe = std::env::current_exe()?;
    if let Some(parent) = exe.parent() {
        let release = parent
            .join("..")
            .join("share")
            .join("rafaello")
            .join("plugins")
            .join(name);
        if release.is_dir() {
            return Ok(release);
        }

        let crate_name = format!("rafaello-{name}");
        let mut cur = Some(parent);
        while let Some(dir) = cur {
            let candidate = dir.join("crates").join(&crate_name);
            if candidate.is_dir() {
                return Ok(candidate);
            }
            cur = dir.parent();
        }
    }

    Err(BundledError::NotFound {
        name: name.to_owned(),
    })
}
