//! `rfl init` subcommand (scope §A1, §A2, PP1).
//!
//! Materialises the default `rafaello.lock` for a fresh project root
//! plus the bundled `rfl-openai` package tree at
//! `${PROJECT_ROOT}/.rafaello/plugins/<topic-id>/`. The package tree
//! is copied with symlinks dereferenced (PP1 containment invariant —
//! `rafaello_core::compile::resolve_entry` rejects symlinks escaping
//! `package_dir`).

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

use chrono::Utc;
use rafaello_core::digest;
use rafaello_core::error::{DigestError, LockError, ManifestError};
use rafaello_core::lock::{
    Bindings, CanonicalId, Grant, GrantBundle, GrantEnv, GrantFilesystem, GrantNetwork, Lock,
    PluginEntry,
};
use rafaello_core::manifest::capabilities::{
    Capabilities as ManifestCapabilities, CapabilityBundle,
};
use rafaello_core::manifest::Manifest;
use rafaello_core::topic_id;

use crate::bundled::{self, BundledError, OPENAI_NAMES};
use crate::pp1;

const OPENAI_CANONICAL: &str = "builtin:openai@0.0.0";

#[derive(Debug, clap::Args)]
pub struct InitArgs {
    #[arg(long, default_value_t = false)]
    pub yes: bool,
    #[arg(long, default_value_t = false)]
    pub force: bool,
    #[arg(long)]
    pub project_root: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("io error on {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("bundled plugin error: {0}")]
    Bundled(#[from] BundledError),
    #[error("manifest error: {0}")]
    Manifest(#[source] Box<ManifestError>),
    #[error("digest error: {0}")]
    Digest(#[from] DigestError),
    #[error("canonical id error: {0}")]
    Canonical(#[source] Box<LockError>),
}

pub fn run(args: InitArgs) -> Result<(), InitError> {
    let project_root = match args.project_root {
        Some(p) => p,
        None => std::env::current_dir().map_err(|source| InitError::Io {
            path: PathBuf::from("."),
            source,
        })?,
    };
    let lock_path = project_root.join("rafaello.lock");
    if lock_path.exists() && !args.force {
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(stderr, "lock already present at {}", lock_path.display());
        let _ = stderr.flush();
        return Ok(());
    }

    let source_dir = bundled::resolve_plugin_dir_for_bundled(&OPENAI_NAMES)?;
    let canonical =
        CanonicalId::parse(OPENAI_CANONICAL).map_err(|e| InitError::Canonical(Box::new(e)))?;

    let manifest_path = source_dir.join("rafaello.toml");
    let manifest_raw = std::fs::read_to_string(&manifest_path).map_err(|source| InitError::Io {
        path: manifest_path.clone(),
        source,
    })?;
    let manifest = Manifest::parse(&manifest_raw).map_err(|e| InitError::Manifest(Box::new(e)))?;
    let grant = synthesise_grant(&manifest);

    if !args.yes && !prompt_accept(&grant)? {
        std::fs::write(&lock_path, EMPTY_LOCK_TOML).map_err(|source| InitError::Io {
            path: lock_path.clone(),
            source,
        })?;
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(
            stderr,
            "declined; wrote empty lock at {}",
            lock_path.display()
        );
        let _ = stderr.flush();
        return Ok(());
    }

    let topic = topic_id::derive(OPENAI_CANONICAL);
    let target_dir =
        pp1::materialise(&project_root, &topic, &source_dir).map_err(|e| InitError::Io {
            path: e.path,
            source: e.source,
        })?;

    let runtime = match bundled::resolve_runtime_binary(&OPENAI_NAMES) {
        Ok(p) => p,
        Err(e) => {
            std::fs::remove_dir_all(&target_dir).ok();
            return Err(InitError::Bundled(e));
        }
    };
    let entry_absolute = target_dir.join(manifest.entry.as_str());
    std::fs::copy(&runtime, &entry_absolute).map_err(|source| InitError::Io {
        path: entry_absolute.clone(),
        source,
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&entry_absolute, std::fs::Permissions::from_mode(0o755)).map_err(
            |source| InitError::Io {
                path: entry_absolute.clone(),
                source,
            },
        )?;
    }

    let content_digest = digest::content_digest(&target_dir)?;
    let manifest_digest = digest::manifest_digest(&manifest.canonical_bytes());

    let bindings = Bindings {
        provider: true,
        provider_id: Some("openai".to_string()),
        load: rafaello_core::lock::LoadPolicy::Eager,
        ..Bindings::default()
    };
    let entry = PluginEntry {
        entry: manifest.entry.clone(),
        digest: content_digest,
        manifest_digest,
        granted_at: Utc::now(),
        grant,
        bindings,
        flags: Default::default(),
    };

    let mut lock = Lock::default();
    lock.plugins.insert(canonical.clone(), entry);
    lock.session.provider_active = Some(canonical.to_string());

    std::fs::write(&lock_path, lock.to_toml()).map_err(|source| InitError::Io {
        path: lock_path.clone(),
        source,
    })?;

    Ok(())
}

const EMPTY_LOCK_TOML: &str = "[session]\n";

fn prompt_accept(grant: &Grant) -> Result<bool, InitError> {
    {
        let mut stderr = std::io::stderr().lock();
        let _ = render_review(&mut stderr, grant);
        let _ = stderr.flush();
    }
    let mut input = String::new();
    match std::io::stdin().read_line(&mut input) {
        Ok(0) => Ok(false),
        Ok(_) => {
            let trimmed = input.trim();
            Ok(matches!(trimmed, "y" | "Y"))
        }
        Err(_) => Ok(false),
    }
}

fn render_review<W: Write>(w: &mut W, grant: &Grant) -> std::io::Result<()> {
    writeln!(w, "rfl init: review default grant for {}", OPENAI_CANONICAL)?;
    let default = grant.bundles.get("default");
    if let Some(net) = default.and_then(|b| b.network.as_ref()) {
        writeln!(
            w,
            "network: mode={:?}, allow_hosts={:?}",
            net.mode, net.allow_hosts
        )?;
    }
    if let Some(env) = default.and_then(|b| b.env.as_ref()) {
        writeln!(
            w,
            "env: pass={:?}, allow_secrets={:?}",
            env.pass, env.allow_secrets
        )?;
        writeln!(w, "env.set: {:?}", env.set)?;
    }
    writeln!(
        w,
        "subscribes: {:?} / publishes: {:?}",
        grant.subscribes, grant.publishes
    )?;
    write!(w, "Proceed? [y/N] ")
}

fn synthesise_grant(manifest: &Manifest) -> Grant {
    let bundles = manifest
        .capabilities
        .as_ref()
        .map(synthesise_bundles)
        .unwrap_or_default();
    let (subs, pubs) = manifest
        .bus
        .as_ref()
        .map(|b| (b.subscribes.clone(), b.publishes.clone()))
        .unwrap_or_default();
    Grant {
        bundles,
        subscribes: subs,
        publishes: pubs,
    }
}

fn synthesise_bundles(caps: &ManifestCapabilities) -> BTreeMap<String, GrantBundle> {
    let mut out = BTreeMap::new();
    for (key, bundle) in caps {
        out.insert(key.clone(), bundle_to_grant(bundle));
    }
    out
}

fn bundle_to_grant(b: &CapabilityBundle) -> GrantBundle {
    let filesystem = b.filesystem.as_ref().and_then(|fs| {
        let g = GrantFilesystem {
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
        };
        let empty = g.read_paths.is_empty()
            && g.read_dirs.is_empty()
            && g.write_paths.is_empty()
            && g.write_dirs.is_empty()
            && g.exec_paths.is_empty()
            && g.exec_dirs.is_empty();
        if empty {
            None
        } else {
            Some(g)
        }
    });
    GrantBundle {
        filesystem,
        network: b.network.as_ref().map(|n| GrantNetwork {
            mode: n.mode,
            allow_hosts: n.allow_hosts.clone(),
        }),
        env: b.env.as_ref().map(|e| GrantEnv {
            pass: e.pass.clone(),
            set: e.set.clone(),
            allow_secrets: e.allow_secrets.clone(),
        }),
        limits: None,
    }
}
