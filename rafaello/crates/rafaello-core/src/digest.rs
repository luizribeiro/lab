//! Deterministic digest computation (scope §D1, §D2, §D3).
//!
//! `content_digest` walks `package_dir` files-only, sorted by relative
//! path normalised to `/`-separators, folding
//! `len_le(path) || path || len_le(contents) || contents` into a
//! single sha256. Symlinks are followed when their canonical target
//! is inside `package_dir`; directory-symlink cycles are caught by a
//! recursion-stack of ancestor canonical paths (a global visited-set
//! would silently skip distinct logical paths sharing a canonical
//! target — e.g. `vendor_src -> src/`). File mode, mtime, and
//! ownership are intentionally excluded.

use std::fs;
use std::path::{Path, PathBuf};

use data_encoding::HEXLOWER;
use sha2::{Digest, Sha256};

use crate::error::DigestError;

pub struct RecomputedDigests {
    pub content: String,
    pub manifest: String,
}

pub fn manifest_digest(canonical_bytes: &[u8]) -> String {
    let digest = Sha256::digest(canonical_bytes);
    format!("sha256:{}", HEXLOWER.encode(&digest))
}

pub fn content_digest(package_dir: &Path) -> Result<String, DigestError> {
    let root_canonical = fs::canonicalize(package_dir)?;
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![root_canonical.clone()];
    walk(&root_canonical, &root_canonical, "", &mut stack, &mut files)?;
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (path, contents) in &files {
        let path_bytes = path.as_bytes();
        hasher.update((path_bytes.len() as u64).to_le_bytes());
        hasher.update(path_bytes);
        hasher.update((contents.len() as u64).to_le_bytes());
        hasher.update(contents);
    }
    Ok(format!("sha256:{}", HEXLOWER.encode(&hasher.finalize())))
}

fn walk(
    real_dir: &Path,
    root_canonical: &Path,
    rel_prefix: &str,
    stack: &mut Vec<PathBuf>,
    files: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), DigestError> {
    let mut entries: Vec<fs::DirEntry> = fs::read_dir(real_dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let name = entry.file_name();
        let name_str = name.to_string_lossy().into_owned();
        let rel = if rel_prefix.is_empty() {
            name_str
        } else {
            format!("{}/{}", rel_prefix, name_str)
        };
        let path = entry.path();
        let lmeta = fs::symlink_metadata(&path)?;

        if lmeta.file_type().is_symlink() {
            let canonical_target = fs::canonicalize(&path)?;
            if !canonical_target.starts_with(root_canonical) {
                return Err(DigestError::SymlinkEscape);
            }
            let tmeta = fs::metadata(&path)?;
            if tmeta.is_dir() {
                if stack.iter().any(|p| p == &canonical_target) {
                    return Err(DigestError::SymlinkCycle);
                }
                stack.push(canonical_target.clone());
                walk(&canonical_target, root_canonical, &rel, stack, files)?;
                stack.pop();
            } else if tmeta.is_file() {
                let contents = fs::read(&path)?;
                files.push((rel, contents));
            }
        } else if lmeta.is_dir() {
            let canonical = fs::canonicalize(&path)?;
            stack.push(canonical);
            walk(&path, root_canonical, &rel, stack, files)?;
            stack.pop();
        } else if lmeta.is_file() {
            let contents = fs::read(&path)?;
            files.push((rel, contents));
        }
    }
    Ok(())
}
