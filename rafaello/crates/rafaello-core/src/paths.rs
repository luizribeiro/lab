//! Path infrastructure shared across manifest validation, V3,
//! trifecta, carve-out, and the grant compiler (scope §M11, §C3).
//!
//! - [`PathContext`] — per-plugin path roots used to expand the
//!   closed §M8 placeholder set.
//! - [`RootKind`] — selects which `PathContext` root a resolved
//!   path is required to live under.
//! - [`resolve_under_root`] — placeholder expansion + walk that
//!   canonicalises the longest existing ancestor (rejecting
//!   symlink escapes), lexically joins the non-existent suffix,
//!   and enforces a final containment check on the named root.

use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

use crate::error::PathError;
use crate::manifest::placeholders;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathContext {
    pub project_root: PathBuf,
    pub home: PathBuf,
    pub plugin_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub state_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RootKind {
    Project,
    Plugin,
}

impl RootKind {
    fn root<'a>(&self, ctx: &'a PathContext) -> &'a Path {
        match self {
            RootKind::Project => &ctx.project_root,
            RootKind::Plugin => &ctx.plugin_dir,
        }
    }
}

/// Resolve a placeholder template into an absolute path required
/// to live under the named root.
///
/// Algorithm (scope §C3):
/// 1. Expand the closed §M8 placeholder set against `ctx`.
/// 2. Walk components, accumulating the longest **existing**
///    ancestor.
/// 3. Canonicalise that ancestor (following symlinks). If the
///    canonical form leaves the root → [`PathError::SymlinkEscape`].
/// 4. Lexically join the non-existent suffix on top, resolving
///    `..` segments without ever popping above the canonical
///    ancestor.
/// 5. Final `starts_with` containment check against the
///    canonical root → [`PathError::PathEscape`] on failure.
pub fn resolve_under_root(
    template: &str,
    ctx: &PathContext,
    root_kind: RootKind,
) -> Result<PathBuf, PathError> {
    let expanded = placeholders::expand_to_path_error(template, ctx)?;
    let path = PathBuf::from(&expanded);
    if !path.is_absolute() {
        return Err(PathError::NotAbsolute);
    }

    let root_substituted = root_kind.root(ctx);
    let root_canon = std::fs::canonicalize(root_substituted)?;

    let lexical_parts = lexically_normalize(&path)?;
    let mut lexical = PathBuf::from("/");
    for s in &lexical_parts {
        lexical.push(s);
    }
    if !lexical.starts_with(root_substituted) && !lexical.starts_with(&root_canon) {
        return Err(PathError::PathEscape);
    }

    let mut existing = PathBuf::from("/");
    let mut suffix: Vec<&OsStr> = Vec::new();
    let mut split = false;
    for s in &lexical_parts {
        if split {
            suffix.push(s);
            continue;
        }
        let candidate = existing.join(s);
        if candidate.symlink_metadata().is_ok() {
            existing = candidate;
        } else {
            split = true;
            suffix.push(s);
        }
    }

    let canon_existing = std::fs::canonicalize(&existing)?;
    if !canon_existing.starts_with(&root_canon) {
        return Err(PathError::SymlinkEscape);
    }

    let mut result = canon_existing;
    for s in suffix {
        result.push(s);
    }
    if !result.starts_with(&root_canon) {
        return Err(PathError::PathEscape);
    }
    Ok(result)
}

fn lexically_normalize(path: &Path) -> Result<Vec<OsString>, PathError> {
    let mut parts: Vec<OsString> = Vec::new();
    for comp in path.components() {
        match comp {
            Component::RootDir | Component::Prefix(_) | Component::CurDir => {}
            Component::Normal(s) => parts.push(s.to_owned()),
            Component::ParentDir => {
                if parts.pop().is_none() {
                    return Err(PathError::PathEscape);
                }
            }
        }
    }
    Ok(parts)
}
