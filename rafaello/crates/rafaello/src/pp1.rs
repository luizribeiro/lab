//! Shared PP1 materialisation helper (scope §"Internal split" — c02 init
//! and c05 install both copy a bundled-or-fixture source tree to
//! `${PROJECT_ROOT}/.rafaello/plugins/<topic-id>/` with symlinks
//! dereferenced).

use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Pp1IoError {
    pub path: PathBuf,
    pub source: std::io::Error,
}

pub fn copy_tree_dereferenced(src: &Path, dst: &Path) -> Result<(), Pp1IoError> {
    std::fs::create_dir_all(dst).map_err(|source| Pp1IoError {
        path: dst.to_path_buf(),
        source,
    })?;
    let entries = std::fs::read_dir(src).map_err(|source| Pp1IoError {
        path: src.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| Pp1IoError {
            path: src.to_path_buf(),
            source,
        })?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let meta = std::fs::metadata(&from).map_err(|source| Pp1IoError {
            path: from.clone(),
            source,
        })?;
        if meta.is_dir() {
            copy_tree_dereferenced(&from, &to)?;
        } else if meta.is_file() {
            std::fs::copy(&from, &to).map_err(|source| Pp1IoError {
                path: from.clone(),
                source,
            })?;
            let mut perms = std::fs::metadata(&to)
                .map_err(|source| Pp1IoError {
                    path: to.clone(),
                    source,
                })?
                .permissions();
            let src_perms = meta.permissions();
            perms.set_readonly(src_perms.readonly());
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                perms.set_mode(src_perms.mode());
            }
            std::fs::set_permissions(&to, perms).map_err(|source| Pp1IoError {
                path: to.clone(),
                source,
            })?;
        }
    }
    Ok(())
}

/// Materialise `source_dir` into `${project_root}/.rafaello/plugins/<topic>/`
/// (removing any prior contents). Returns the target dir.
pub fn materialise(
    project_root: &Path,
    topic: &str,
    source_dir: &Path,
) -> Result<PathBuf, Pp1IoError> {
    let plugins_root = project_root.join(".rafaello").join("plugins");
    std::fs::create_dir_all(&plugins_root).map_err(|source| Pp1IoError {
        path: plugins_root.clone(),
        source,
    })?;
    let target_dir = plugins_root.join(topic);
    if target_dir.exists() {
        std::fs::remove_dir_all(&target_dir).map_err(|source| Pp1IoError {
            path: target_dir.clone(),
            source,
        })?;
    }
    copy_tree_dereferenced(source_dir, &target_dir)?;
    Ok(target_dir)
}
