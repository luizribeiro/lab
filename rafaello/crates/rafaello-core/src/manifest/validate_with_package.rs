//! Package-level validation (scope §M10).
//!
//! Layered on top of V1's grammar checks: invokes
//! [`crate::validate::manifest_standalone`] first, then performs
//! every check that requires the on-disk package layout —
//! `openrpc.json` sibling presence, `entry` resolution +
//! file-vs-dir + escape, `grant_match` resolution + presence, and
//! the syntactic refusal of `${project}`-anchored
//! `exec_paths` / `exec_dirs` per scope §V1 + security RFC §6.9.
//! The full resolve-against-real-project check lives in V3 (c27).

use std::path::Path;

use crate::error::ManifestError;
use crate::manifest::capability_path_template::CapabilityPathTemplate;
use crate::manifest::top_level::Manifest;

pub fn validate_with_package(
    _manifest_path: &Path,
    package_dir: &Path,
    manifest: &Manifest,
) -> Result<(), ManifestError> {
    crate::validate::manifest_standalone(manifest).map_err(ManifestError::Validation)?;

    check_openrpc_sibling(package_dir)?;

    let pkg_canon = std::fs::canonicalize(package_dir)?;

    resolve_inside_package(
        &pkg_canon,
        package_dir,
        manifest.entry.as_str(),
        PackagePathKind::Entry,
    )?;

    if let Some(provides) = &manifest.provides {
        for meta in provides.tool.values() {
            if let Some(gm) = &meta.grant_match {
                resolve_inside_package(
                    &pkg_canon,
                    package_dir,
                    gm.as_str(),
                    PackagePathKind::GrantMatch,
                )?;
            }
        }
    }

    check_exec_paths(manifest)?;

    Ok(())
}

fn check_openrpc_sibling(package_dir: &Path) -> Result<(), ManifestError> {
    let openrpc = package_dir.join("openrpc.json");
    let meta = match std::fs::symlink_metadata(&openrpc) {
        Ok(m) => m,
        Err(_) => return Err(ManifestError::MissingOpenRpc),
    };
    let file_type = meta.file_type();
    if file_type.is_symlink() {
        match std::fs::metadata(&openrpc) {
            Ok(m) if m.is_file() => Ok(()),
            _ => Err(ManifestError::MissingOpenRpc),
        }
    } else if file_type.is_file() {
        Ok(())
    } else {
        Err(ManifestError::MissingOpenRpc)
    }
}

#[derive(Clone, Copy)]
enum PackagePathKind {
    Entry,
    GrantMatch,
}

impl PackagePathKind {
    fn escape(self) -> ManifestError {
        match self {
            Self::Entry => ManifestError::EntryEscape,
            Self::GrantMatch => ManifestError::GrantMatchEscape,
        }
    }
    fn not_found(self) -> ManifestError {
        match self {
            Self::Entry => ManifestError::EntryNotFound,
            Self::GrantMatch => ManifestError::GrantMatchNotFound,
        }
    }
    fn not_file(self) -> ManifestError {
        match self {
            Self::Entry => ManifestError::EntryNotFile,
            Self::GrantMatch => ManifestError::GrantMatchNotFile,
        }
    }
}

fn resolve_inside_package(
    pkg_canon: &Path,
    package_dir: &Path,
    rel: &str,
    kind: PackagePathKind,
) -> Result<(), ManifestError> {
    let joined = package_dir.join(rel);
    if std::fs::symlink_metadata(&joined).is_err() {
        return Err(kind.not_found());
    }
    let canon = match std::fs::canonicalize(&joined) {
        Ok(c) => c,
        Err(_) => return Err(kind.not_found()),
    };
    if !canon.starts_with(pkg_canon) {
        return Err(kind.escape());
    }
    if !canon.is_file() {
        return Err(kind.not_file());
    }
    Ok(())
}

fn check_exec_paths(manifest: &Manifest) -> Result<(), ManifestError> {
    let Some(caps) = &manifest.capabilities else {
        return Ok(());
    };
    for bundle in caps.values() {
        let Some(fs) = &bundle.filesystem else {
            continue;
        };
        for tpl in fs.exec_paths.iter().chain(fs.exec_dirs.iter()) {
            if starts_with_project(tpl) {
                return Err(ManifestError::ExecPathInsideProject);
            }
        }
    }
    Ok(())
}

fn starts_with_project(tpl: &CapabilityPathTemplate) -> bool {
    let s = tpl.as_str();
    s == "${project}" || s.starts_with("${project}/")
}
