//! `rfl install --fixture <PACKAGE_DIR>` subcommand (scope §Tr1).
//!
//! Installs a local package fixture into `${PROJECT_ROOT}/rafaello.lock`
//! after running m1's V3 path (`validate::lock`, which internally
//! invokes `trifecta::evaluate` + `carveout::*`). Trifecta refusal and
//! carve-out refusal map to typed install errors with stderr hints;
//! the `--i-know-what-im-doing` / `--allow-credential-paths` flags are
//! applied to the candidate `PluginEntry` *before* validation runs.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use chrono::Utc;
use rafaello_core::audit::{AuditError, AuditKind, AuditWriter};
use rafaello_core::digest;
use rafaello_core::error::{DigestError, LockError, ManifestError, ValidationError};
use rafaello_core::lock::{
    Bindings, CanonicalId, Grant, GrantBundle, GrantEnv, GrantFilesystem, GrantLimits,
    GrantNetwork, Lock, LockFlags, PluginEntry, ToolMeta,
};
use rafaello_core::manifest::capabilities::{
    Capabilities as ManifestCapabilities, CapabilityBundle,
};
use rafaello_core::manifest::{validate_with_package, Manifest};
use rafaello_core::paths::PathContext;
use rafaello_core::sinks;
use rafaello_core::topic_id;
use rafaello_core::trifecta;
use rafaello_core::validate::{self, LockValidationContext};
use serde_json::json;

#[derive(Debug, clap::Args)]
pub struct InstallArgs {
    #[arg(long)]
    pub fixture: PathBuf,
    #[arg(long)]
    pub lock: Option<PathBuf>,
    #[arg(long = "i-know-what-im-doing", default_value_t = false)]
    pub i_know_what_im_doing: bool,
    #[arg(long = "allow-credential-paths", default_value_t = false)]
    pub allow_credential_paths: bool,
    #[arg(long, default_value_t = false)]
    pub verbose: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("io error on {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("manifest error: {0}")]
    Manifest(#[source] Box<ManifestError>),
    #[error("digest error: {0}")]
    Digest(#[from] DigestError),
    #[error("lock parse error: {0}")]
    LockParse(#[source] Box<LockError>),
    #[error("canonical id error: {0}")]
    Canonical(#[source] Box<LockError>),
    #[error(
        "TrifectaRefused(reads_untrusted={reads}, has_outbound={outbound}, has_workspace_write={write})"
    )]
    TrifectaRefused {
        reads: bool,
        outbound: bool,
        write: bool,
    },
    #[error("CarveOutRefused")]
    CarveOutRefused,
    #[error("Validation: {0}")]
    Validation(#[source] Box<ValidationError>),
    #[error("audit: {0}")]
    Audit(#[from] AuditError),
    #[error("NoHomeDir")]
    NoHomeDir,
}

pub fn run(args: InstallArgs) -> Result<(), InstallError> {
    let project_root = std::env::current_dir().map_err(|source| InstallError::Io {
        path: PathBuf::from("."),
        source,
    })?;
    let package_dir = args
        .fixture
        .canonicalize()
        .map_err(|source| InstallError::Io {
            path: args.fixture.clone(),
            source,
        })?;
    let lock_path = args
        .lock
        .clone()
        .unwrap_or_else(|| project_root.join("rafaello.lock"));

    let audit = AuditWriter::open_for_install(&project_root)?;

    let manifest_path = package_dir.join("rafaello.toml");
    let manifest_raw =
        std::fs::read_to_string(&manifest_path).map_err(|source| InstallError::Io {
            path: manifest_path.clone(),
            source,
        })?;
    let manifest =
        Manifest::parse(&manifest_raw).map_err(|e| InstallError::Manifest(Box::new(e)))?;
    validate_with_package(&manifest_path, &package_dir, &manifest)
        .map_err(|e| InstallError::Manifest(Box::new(e)))?;

    let canonical_str = format!("local:{}@{}", manifest.name, manifest.version);
    let canonical =
        CanonicalId::parse(&canonical_str).map_err(|e| InstallError::Canonical(Box::new(e)))?;

    let manifest_digest = digest::manifest_digest(&manifest.canonical_bytes());
    let content_digest = digest::content_digest(&package_dir)?;

    let bundles = synthesise_bundles(manifest.capabilities.as_ref());
    let (bus_subs, bus_pubs) = manifest
        .bus
        .as_ref()
        .map(|b| (b.subscribes.clone(), b.publishes.clone()))
        .unwrap_or_default();
    let grant = Grant {
        bundles,
        subscribes: bus_subs,
        publishes: bus_pubs,
    };
    let bindings = synthesise_bindings(&manifest, &grant);
    let flags = LockFlags {
        i_know_what_im_doing: args.i_know_what_im_doing,
        allow_credential_paths: args.allow_credential_paths,
    };
    let candidate = PluginEntry {
        entry: manifest.entry.clone(),
        digest: content_digest,
        manifest_digest,
        granted_at: Utc::now(),
        grant,
        bindings,
        flags,
    };

    expose_candidate(&candidate);

    let mut merged = match std::fs::read_to_string(&lock_path) {
        Ok(s) => Lock::from_toml(&s).map_err(|e| InstallError::LockParse(Box::new(e)))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Lock::default(),
        Err(source) => {
            return Err(InstallError::Io {
                path: lock_path.clone(),
                source,
            });
        }
    };
    merged.plugins.insert(canonical.clone(), candidate.clone());

    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(InstallError::NoHomeDir)?;
    let install_root = project_root.join(".rafaello").join("plugins");
    let mut plugin_dirs: BTreeMap<CanonicalId, PathBuf> = BTreeMap::new();
    for c in merged.plugins.keys() {
        let dir = if c == &canonical {
            package_dir.clone()
        } else {
            install_root.join(topic_id::derive(&c.to_string()))
        };
        plugin_dirs.insert(c.clone(), dir);
    }
    let val_ctx = LockValidationContext {
        project_root: project_root.clone(),
        home: home.clone(),
        plugin_dirs: plugin_dirs.clone(),
        cache_root: project_root.join(".rafaello").join("cache"),
        state_root: project_root.join(".rafaello").join("state"),
    };

    if let Err(err) = validate::lock(&merged, &val_ctx) {
        match err {
            ValidationError::TrifectaRefused {
                reads_untrusted,
                has_outbound,
                has_workspace_write,
            } => {
                eprintln!(
                    "TrifectaRefused(reads_untrusted={}, has_outbound={}, has_workspace_write={})",
                    reads_untrusted, has_outbound, has_workspace_write
                );
                eprintln!("hint: pass --i-know-what-im-doing to override (security RFC §7.1)");
                let _ = audit.record(
                    AuditKind::InstallRefused,
                    None,
                    &json!({
                        "canonical": canonical.to_string(),
                        "reads_untrusted": reads_untrusted,
                        "has_outbound": has_outbound,
                        "has_workspace_write": has_workspace_write,
                    }),
                )?;
                return Err(InstallError::TrifectaRefused {
                    reads: reads_untrusted,
                    outbound: has_outbound,
                    write: has_workspace_write,
                });
            }
            ValidationError::CarveOutRefused | ValidationError::CarveOutTooLarge => {
                eprintln!("CarveOutRefused: credential-path carve-out not allowed");
                eprintln!("hint: pass --allow-credential-paths to override (security RFC §7.3)");
                let _ = audit.record(
                    AuditKind::InstallRefused,
                    None,
                    &json!({
                        "canonical": canonical.to_string(),
                        "reason": "carve_out_refused",
                    }),
                )?;
                return Err(InstallError::CarveOutRefused);
            }
            other => return Err(InstallError::Validation(Box::new(other))),
        }
    }

    if args.verbose {
        let path_ctx = PathContext {
            project_root: project_root.clone(),
            home,
            plugin_dir: package_dir.clone(),
            cache_dir: project_root.join(".rafaello").join("cache"),
            state_dir: project_root.join(".rafaello").join("state"),
        };
        let state = trifecta::evaluate(&merged, &canonical, &path_ctx);
        eprintln!(
            "trifecta diagnostic: reads_untrusted={} has_outbound={} has_workspace_write={}",
            state.reads_untrusted, state.has_outbound, state.has_workspace_write
        );
    }

    let mut unused: Vec<String> = Vec::new();
    let mut all_secrets: BTreeSet<String> = BTreeSet::new();
    for bundle in candidate.grant.bundles.values() {
        let Some(env) = &bundle.env else { continue };
        let pass: BTreeSet<&str> = env.pass.iter().map(String::as_str).collect();
        for name in &env.allow_secrets {
            all_secrets.insert(name.clone());
            if !pass.contains(name.as_str()) && !unused.contains(name) {
                eprintln!(
                    "warning: unused allow_secrets entry '{}' (no matching env.pass entry)",
                    name
                );
                unused.push(name.clone());
            }
        }
    }

    std::fs::write(&lock_path, merged.to_toml()).map_err(|source| InstallError::Io {
        path: lock_path.clone(),
        source,
    })?;

    let kind = if candidate.flags.i_know_what_im_doing {
        AuditKind::TrifectaOverridden
    } else {
        AuditKind::InstallAccepted
    };
    let mut details = serde_json::Map::new();
    details.insert("canonical".to_string(), json!(canonical.to_string()));
    if !all_secrets.is_empty() {
        let list: Vec<String> = all_secrets.iter().cloned().collect();
        details.insert("allow_secrets".to_string(), json!(list));
    }
    details.insert("unused_allow_secrets".to_string(), json!(unused));
    audit.record(kind, None, &serde_json::Value::Object(details))?;
    Ok(())
}

fn synthesise_bundles(caps: Option<&ManifestCapabilities>) -> BTreeMap<String, GrantBundle> {
    let mut out = BTreeMap::new();
    let Some(caps) = caps else { return out };
    for (key, bundle) in caps {
        out.insert(key.clone(), bundle_to_grant(bundle));
    }
    out
}

fn bundle_to_grant(b: &CapabilityBundle) -> GrantBundle {
    GrantBundle {
        filesystem: b.filesystem.as_ref().map(|fs| GrantFilesystem {
            read_paths: fs
                .read_paths
                .iter()
                .map(|t| t.as_str().to_string())
                .collect(),
            read_dirs: fs
                .read_dirs
                .iter()
                .map(|t| t.as_str().to_string())
                .collect(),
            write_paths: fs
                .write_paths
                .iter()
                .map(|t| t.as_str().to_string())
                .collect(),
            write_dirs: fs
                .write_dirs
                .iter()
                .map(|t| t.as_str().to_string())
                .collect(),
            exec_paths: fs
                .exec_paths
                .iter()
                .map(|t| t.as_str().to_string())
                .collect(),
            exec_dirs: fs
                .exec_dirs
                .iter()
                .map(|t| t.as_str().to_string())
                .collect(),
        }),
        network: b.network.as_ref().map(|n| GrantNetwork {
            mode: n.mode,
            allow_hosts: n.allow_hosts.clone(),
        }),
        env: b.env.as_ref().map(|e| GrantEnv {
            pass: e.pass.clone(),
            set: e.set.clone(),
            allow_secrets: e.allow_secrets.clone(),
        }),
        limits: b.limits.as_ref().map(|l| GrantLimits {
            max_cpu_time: l.max_cpu_time,
            max_open_files: l.max_open_files,
            max_address_space: l.max_address_space,
            max_processes: l.max_processes,
        }),
    }
}

fn synthesise_bindings(manifest: &Manifest, grant: &Grant) -> Bindings {
    let mut bindings = Bindings::default();
    let Some(p) = manifest.provides.as_ref() else {
        return bindings;
    };
    bindings.tools = p.tools.clone();
    bindings.provider_id = p.provider.clone();
    bindings.provider = p.provider.is_some();
    for (name, meta) in &p.tool {
        let sinks_inferred = meta.sinks.is_none();
        let sinks_list = match &meta.sinks {
            Some(s) => s.clone(),
            None => {
                let effective = sinks::effective_grant(grant, name);
                sinks::infer_defaults(&effective, &None)
            }
        };
        bindings.tool_meta.insert(
            name.clone(),
            ToolMeta {
                sinks: sinks_list,
                sinks_inferred,
                grant_match: meta.grant_match.clone(),
                always_confirm: meta.always_confirm,
            },
        );
    }
    bindings.renderer_kinds = manifest.renderers.iter().map(|r| r.kind.clone()).collect();
    bindings
}

fn expose_candidate(entry: &PluginEntry) {
    *CANDIDATE.lock().unwrap() = Some(entry.clone());
}

static CANDIDATE: std::sync::Mutex<Option<PluginEntry>> = std::sync::Mutex::new(None);

/// Test accessor exposing the candidate `PluginEntry` at the moment
/// `validate::lock` ran. Always available so integration tests can
/// inspect override-flag plumbing without rebuilding with cfg(test).
pub fn test_candidate() -> Option<PluginEntry> {
    CANDIDATE.lock().unwrap().clone()
}
