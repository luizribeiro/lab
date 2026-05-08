//! Carve-out grant decomposition (scope §K1–§K4).
//!
//! `compile_against` walks a `GrantBundle`'s filesystem entries
//! against [`CARVE_OUTS`] and produces a [`DecomposedGrant`]:
//! project-class reads of an ancestor are decomposed into the
//! ancestor's immediate non-hidden children minus the carve-out
//! leaves (capped at 256); credential-class reads, all writes,
//! and any explicit leaf hit refuse with
//! [`CompileError::CarveOutRefused`]. The `allow_credential_paths`
//! override emits the broad grant verbatim and records the flag.

use std::path::{Path, PathBuf};

use crate::error::CompileError;
use crate::lock::{CanonicalId, GrantBundle};
use crate::manifest::placeholders;
use crate::paths::PathContext;

const DECOMPOSE_CAP: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CarveOutClass {
    Project,
    Credential,
}

#[derive(Debug, Clone, Copy)]
pub struct CarveOut {
    pub template: &'static str,
    pub class: CarveOutClass,
}

pub const CARVE_OUTS: &[CarveOut] = &[
    CarveOut {
        template: "${project}/rafaello.lock",
        class: CarveOutClass::Project,
    },
    CarveOut {
        template: "${project}/.rafaello",
        class: CarveOutClass::Project,
    },
    CarveOut {
        template: "${home}/.config/rafaello",
        class: CarveOutClass::Credential,
    },
    CarveOut {
        template: "${home}/.ssh",
        class: CarveOutClass::Credential,
    },
    CarveOut {
        template: "${home}/.gnupg",
        class: CarveOutClass::Credential,
    },
    CarveOut {
        template: "${home}/.aws",
        class: CarveOutClass::Credential,
    },
    CarveOut {
        template: "${home}/.config/gh",
        class: CarveOutClass::Credential,
    },
    CarveOut {
        template: "${home}/.netrc",
        class: CarveOutClass::Credential,
    },
];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DecomposedFlags {
    pub allow_credential_paths_active: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DecomposedGrant {
    pub read_dirs: Vec<PathBuf>,
    pub read_paths: Vec<PathBuf>,
    pub write_dirs: Vec<PathBuf>,
    pub write_paths: Vec<PathBuf>,
    pub flags: DecomposedFlags,
}

struct ResolvedCarveOut {
    path: PathBuf,
    class: CarveOutClass,
}

#[derive(PartialEq, Eq)]
enum Relation {
    Unrelated,
    Equal,
    EntryInsideCarveOut,
    CarveOutInsideEntry,
}

pub fn compile_against(
    grant: &GrantBundle,
    _canonical: &CanonicalId,
    ctx: &PathContext,
    allow_credential_paths: bool,
) -> Result<DecomposedGrant, CompileError> {
    let resolved = resolve_carveouts(ctx)?;
    let mut out = DecomposedGrant {
        flags: DecomposedFlags {
            allow_credential_paths_active: allow_credential_paths,
        },
        ..DecomposedGrant::default()
    };

    let Some(fs) = &grant.filesystem else {
        return Ok(out);
    };

    if allow_credential_paths {
        out.read_dirs = expand_all(&fs.read_dirs, ctx)?;
        out.read_paths = expand_all(&fs.read_paths, ctx)?;
        out.write_dirs = expand_all(&fs.write_dirs, ctx)?;
        out.write_paths = expand_all(&fs.write_paths, ctx)?;
        return Ok(out);
    }

    for template in &fs.read_dirs {
        let entry = expand_path(template, ctx)?;
        let mut decompose = false;
        for co in &resolved {
            match relate(&entry, &co.path) {
                Relation::Unrelated => {}
                Relation::Equal | Relation::EntryInsideCarveOut => {
                    return Err(CompileError::CarveOutRefused);
                }
                Relation::CarveOutInsideEntry => match co.class {
                    CarveOutClass::Credential => return Err(CompileError::CarveOutRefused),
                    CarveOutClass::Project => decompose = true,
                },
            }
        }
        if decompose {
            decompose_dir(&entry, &resolved, &mut out.read_dirs)?;
        } else {
            out.read_dirs.push(entry);
        }
    }

    for template in &fs.read_paths {
        let entry = expand_path(template, ctx)?;
        for co in &resolved {
            match relate(&entry, &co.path) {
                Relation::Equal | Relation::EntryInsideCarveOut => {
                    return Err(CompileError::CarveOutRefused);
                }
                Relation::Unrelated | Relation::CarveOutInsideEntry => {}
            }
        }
        out.read_paths.push(entry);
    }

    for template in &fs.write_dirs {
        let entry = expand_path(template, ctx)?;
        if touches_any(&entry, &resolved) {
            return Err(CompileError::CarveOutRefused);
        }
        out.write_dirs.push(entry);
    }

    for template in &fs.write_paths {
        let entry = expand_path(template, ctx)?;
        if touches_any(&entry, &resolved) {
            return Err(CompileError::CarveOutRefused);
        }
        out.write_paths.push(entry);
    }

    Ok(out)
}

fn resolve_carveouts(ctx: &PathContext) -> Result<Vec<ResolvedCarveOut>, CompileError> {
    CARVE_OUTS
        .iter()
        .map(|c| {
            expand_path(c.template, ctx).map(|p| ResolvedCarveOut {
                path: p,
                class: c.class,
            })
        })
        .collect()
}

fn expand_all(templates: &[String], ctx: &PathContext) -> Result<Vec<PathBuf>, CompileError> {
    templates.iter().map(|t| expand_path(t, ctx)).collect()
}

fn expand_path(template: &str, ctx: &PathContext) -> Result<PathBuf, CompileError> {
    placeholders::expand(template, ctx)
        .map(PathBuf::from)
        .map_err(|_| CompileError::UnknownPlaceholder)
}

fn relate(entry: &Path, carveout: &Path) -> Relation {
    if entry == carveout {
        Relation::Equal
    } else if entry.starts_with(carveout) {
        Relation::EntryInsideCarveOut
    } else if carveout.starts_with(entry) {
        Relation::CarveOutInsideEntry
    } else {
        Relation::Unrelated
    }
}

fn touches_any(entry: &Path, carveouts: &[ResolvedCarveOut]) -> bool {
    carveouts
        .iter()
        .any(|co| relate(entry, &co.path) != Relation::Unrelated)
}

fn decompose_dir(
    entry: &Path,
    carveouts: &[ResolvedCarveOut],
    out: &mut Vec<PathBuf>,
) -> Result<(), CompileError> {
    let mut children: Vec<PathBuf> = Vec::new();
    for item in std::fs::read_dir(entry)? {
        let item = item?;
        let name = item.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if name_str.starts_with('.') {
            continue;
        }
        let child = item.path();
        if carveouts
            .iter()
            .any(|co| co.class == CarveOutClass::Project && co.path == child)
        {
            continue;
        }
        children.push(child);
    }
    if children.len() > DECOMPOSE_CAP {
        return Err(CompileError::CarveOutTooLarge);
    }
    children.sort();
    out.extend(children);
    Ok(())
}
