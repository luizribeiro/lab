//! Compile-side plan types and entry point per scope §C1, §C2.
//!
//! `CompiledPlugin` is the structured plan m1 hands m2's plugin
//! supervisor; m2 picks the application order.
//!
//! `compile_plugin` carries the §C1.1 precondition contract: a
//! prior successful `validate::lock(..)` is required. The body
//! spot-checks a handful of obvious V3 invariants and returns
//! `CompileError::ValidationNotRun` if any are violated; it does
//! **not** re-run V3. Per-section emitters (bundle flatten, path
//! resolution, carve-out, network/env, entry resolution + digest
//! gating) land in c30–c34.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::carveout;
use crate::digest::RecomputedDigests;
use crate::error::{CompileError, PathError};
use crate::lock::canonical_id::CanonicalId;
use crate::lock::load_policy::LoadPolicy;
use crate::lock::{
    Grant, GrantBundle, GrantEnv, GrantFilesystem, GrantLimits, GrantNetwork, Lock, PluginEntry,
};
use crate::manifest::capabilities::NetworkMode;
use crate::manifest::placeholders;
use crate::paths::{self, PathContext, RootKind};
use crate::topic_id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledPlugin {
    pub canonical: CanonicalId,
    pub topic_id: String,
    pub entry_absolute: PathBuf,
    pub filesystem: FilesystemPlan,
    pub network: NetworkPlan,
    pub env: EnvPlan,
    pub limits: LimitsPlan,
    pub subscribe_patterns: Vec<String>,
    pub publish_topics: Vec<String>,
    pub auto_subscribes: Vec<String>,
    pub tool_meta: BTreeMap<String, ToolMeta>,
    pub provider_id: Option<String>,
    pub load: LoadPolicy,
    pub flags: CompiledFlags,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FilesystemPlan {
    pub read_paths: Vec<PathBuf>,
    pub read_dirs: Vec<PathBuf>,
    pub write_paths: Vec<PathBuf>,
    pub write_dirs: Vec<PathBuf>,
    pub exec_paths: Vec<PathBuf>,
    pub exec_dirs: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkPlan {
    Deny,
    AllowAll,
    Proxy { allow_hosts: Vec<String> },
}

impl Default for NetworkPlan {
    fn default() -> Self {
        NetworkPlan::Deny
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvPlan {
    pub pass: Vec<String>,
    pub set: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LimitsPlan {
    pub max_cpu_time: u64,
    pub max_open_files: u64,
    pub max_address_space: Option<u64>,
    pub max_processes: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompiledFlags {
    pub i_know_what_im_doing: bool,
    pub allow_credential_paths: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolMeta {
    pub sinks: Vec<String>,
    pub sinks_inferred: bool,
    pub grant_match: Option<PathBuf>,
    pub always_confirm: bool,
}

/// Compile a single plugin's spawn-time policy.
///
/// **Precondition (§C1.1).** A prior successful
/// `validate::lock(lock, ..)` on the same `Lock` value. When this
/// function detects a state V3 should have rejected — duplicate
/// topic-id, conflicting tool name without `session.tool_owner`
/// resolution, or a publish on a reserved/foreign namespace — it
/// returns [`CompileError::ValidationNotRun`]. It does not re-run
/// V3 itself.
///
/// Currently wires §C2 bundle flatten + §C3 placeholder
/// resolution + §K carve-out decomposition + §C5 private-state
/// injection. Network/env emission + entry resolution + digest
/// gating + tool_meta projection land in later commits.
pub fn compile_plugin(
    lock: &Lock,
    canonical: &CanonicalId,
    ctx: &PathContext,
    _recomputed_digests: &RecomputedDigests,
) -> Result<CompiledPlugin, CompileError> {
    let entry = lock
        .plugins
        .get(canonical)
        .ok_or(CompileError::ValidationNotRun)?;

    spot_check_v3(lock, canonical, entry)?;

    let topic_id = topic_id::derive(&canonical.to_string());
    let entry_absolute = ctx.plugin_dir.join(entry.entry.as_ref());

    let eff = effective_grant(&entry.grant);

    let resolved_fs = resolve_filesystem(&eff.filesystem, ctx)?;

    let proxy_bundle = GrantBundle {
        filesystem: Some(resolved_fs.clone()),
        ..GrantBundle::default()
    };
    let decomposed = carveout::compile_against(
        &proxy_bundle,
        canonical,
        ctx,
        entry.flags.allow_credential_paths,
    )?;

    let private_state = ctx
        .project_root
        .join(".rafaello-plugin-data")
        .join(&topic_id);

    let mut filesystem = FilesystemPlan {
        read_paths: decomposed.read_paths,
        read_dirs: decomposed.read_dirs,
        write_paths: decomposed.write_paths,
        write_dirs: decomposed.write_dirs,
        exec_paths: resolved_fs.exec_paths.iter().map(PathBuf::from).collect(),
        exec_dirs: resolved_fs.exec_dirs.iter().map(PathBuf::from).collect(),
    };
    filesystem.read_dirs.push(private_state.clone());
    filesystem.write_dirs.push(private_state);
    sort_dedup_paths(&mut filesystem.read_paths);
    sort_dedup_paths(&mut filesystem.read_dirs);
    sort_dedup_paths(&mut filesystem.write_paths);
    sort_dedup_paths(&mut filesystem.write_dirs);
    sort_dedup_paths(&mut filesystem.exec_paths);
    sort_dedup_paths(&mut filesystem.exec_dirs);

    let network = match eff.network.mode {
        NetworkMode::Deny => NetworkPlan::Deny,
        NetworkMode::AllowAll => NetworkPlan::AllowAll,
        NetworkMode::Proxy => NetworkPlan::Proxy {
            allow_hosts: eff.network.allow_hosts,
        },
    };

    let env = EnvPlan {
        pass: eff.env.pass,
        set: eff.env.set,
    };

    let limits = LimitsPlan {
        max_cpu_time: eff.limits.max_cpu_time.unwrap_or(0),
        max_open_files: eff.limits.max_open_files.unwrap_or(0),
        max_address_space: eff.limits.max_address_space,
        max_processes: eff.limits.max_processes,
    };

    Ok(CompiledPlugin {
        canonical: canonical.clone(),
        topic_id,
        entry_absolute,
        filesystem,
        network,
        env,
        limits,
        subscribe_patterns: Vec::new(),
        publish_topics: Vec::new(),
        auto_subscribes: Vec::new(),
        tool_meta: BTreeMap::new(),
        provider_id: entry.bindings.provider_id.clone(),
        load: entry.bindings.load.clone(),
        flags: CompiledFlags {
            i_know_what_im_doing: entry.flags.i_know_what_im_doing,
            allow_credential_paths: entry.flags.allow_credential_paths,
        },
    })
}

/// Spawn-time effective grant per scope §C2: `default` ∪ every
/// named bundle in `grant.bundles` flattened into a single value,
/// with C4 post-flatten ordering applied (sort by string value,
/// dedup) so plans compare byte-equal regardless of bundle
/// iteration order. Capability path strings are still raw —
/// placeholder substitution + path-escape resolution land in c31.
///
/// **Network mode union:** most-permissive wins
/// (`Deny < Proxy < AllowAll`). Under `Proxy`, `allow_hosts` is
/// the sorted+deduped union across every contributing bundle —
/// pi review-4 finding 1's "single spawn-time policy, no per-call
/// switch" model.
///
/// **`env.set` merge:** `BTreeMap` insert across bundles in
/// `grant.bundles`'s natural (key-sorted) order. A duplicate key
/// across bundles takes the last-iterated value; collisions are
/// not expected in well-formed locks but the rule is deterministic
/// regardless.
///
/// **Limits union:** maximum of present values per field. `None`
/// means "this bundle didn't constrain"; defaults (300s cpu, 1024
/// fds per §C6) are c34's job, not c30's.
pub(crate) struct EffectiveGrant {
    pub filesystem: GrantFilesystem,
    pub network: GrantNetwork,
    pub env: GrantEnv,
    pub limits: GrantLimits,
}

pub(crate) fn effective_grant(grant: &Grant) -> EffectiveGrant {
    let mut fs = GrantFilesystem::default();
    let mut net_mode = NetworkMode::Deny;
    let mut allow_hosts: Vec<String> = Vec::new();
    let mut env_pass: Vec<String> = Vec::new();
    let mut env_set: BTreeMap<String, String> = BTreeMap::new();
    let mut limits = GrantLimits::default();

    for bundle in grant.bundles.values() {
        if let Some(f) = &bundle.filesystem {
            fs.read_paths.extend(f.read_paths.iter().cloned());
            fs.read_dirs.extend(f.read_dirs.iter().cloned());
            fs.write_paths.extend(f.write_paths.iter().cloned());
            fs.write_dirs.extend(f.write_dirs.iter().cloned());
            fs.exec_paths.extend(f.exec_paths.iter().cloned());
            fs.exec_dirs.extend(f.exec_dirs.iter().cloned());
        }
        if let Some(n) = &bundle.network {
            net_mode = most_permissive_mode(net_mode, n.mode);
            allow_hosts.extend(n.allow_hosts.iter().cloned());
        }
        if let Some(e) = &bundle.env {
            env_pass.extend(e.pass.iter().cloned());
            for (k, v) in &e.set {
                env_set.insert(k.clone(), v.clone());
            }
        }
        if let Some(l) = &bundle.limits {
            limits.max_cpu_time = max_opt(limits.max_cpu_time, l.max_cpu_time);
            limits.max_open_files = max_opt(limits.max_open_files, l.max_open_files);
            limits.max_address_space = max_opt(limits.max_address_space, l.max_address_space);
            limits.max_processes = max_opt(limits.max_processes, l.max_processes);
        }
    }

    sort_dedup(&mut fs.read_paths);
    sort_dedup(&mut fs.read_dirs);
    sort_dedup(&mut fs.write_paths);
    sort_dedup(&mut fs.write_dirs);
    sort_dedup(&mut fs.exec_paths);
    sort_dedup(&mut fs.exec_dirs);
    sort_dedup(&mut allow_hosts);
    sort_dedup(&mut env_pass);

    EffectiveGrant {
        filesystem: fs,
        network: GrantNetwork {
            mode: net_mode,
            allow_hosts,
        },
        env: GrantEnv {
            pass: env_pass,
            set: env_set,
        },
        limits,
    }
}

fn sort_dedup(v: &mut Vec<String>) {
    v.sort();
    v.dedup();
}

fn sort_dedup_paths(v: &mut Vec<PathBuf>) {
    v.sort();
    v.dedup();
}

/// Resolve every capability path template in `fs` to an absolute
/// path per scope §C3: closed §M8 placeholder expansion + the
/// existing-ancestor-canonical / lexical-suffix / containment
/// resolver from `paths::resolve_under_root` for `${project}` and
/// `${plugin}` prefixed templates. Templates rooted elsewhere
/// (`${home}`, `${cache}`, `${state}`, absolute) are placeholder-
/// expanded only — the post-expansion containment check applies
/// only to project / plugin roots.
fn resolve_filesystem(
    fs: &GrantFilesystem,
    ctx: &PathContext,
) -> Result<GrantFilesystem, CompileError> {
    Ok(GrantFilesystem {
        read_paths: resolve_each(&fs.read_paths, ctx)?,
        read_dirs: resolve_each(&fs.read_dirs, ctx)?,
        write_paths: resolve_each(&fs.write_paths, ctx)?,
        write_dirs: resolve_each(&fs.write_dirs, ctx)?,
        exec_paths: resolve_each(&fs.exec_paths, ctx)?,
        exec_dirs: resolve_each(&fs.exec_dirs, ctx)?,
    })
}

fn resolve_each(items: &[String], ctx: &PathContext) -> Result<Vec<String>, CompileError> {
    items
        .iter()
        .map(|t| resolve_one(t, ctx).map(|p| p.to_string_lossy().into_owned()))
        .collect()
}

fn resolve_one(template: &str, ctx: &PathContext) -> Result<PathBuf, CompileError> {
    if let Some(kind) = root_kind_for(template) {
        paths::resolve_under_root(template, ctx, kind).map_err(map_path_err)
    } else {
        let expanded = placeholders::expand(template, ctx)
            .map_err(|_| CompileError::UnknownPlaceholder)?;
        Ok(PathBuf::from(expanded))
    }
}

fn root_kind_for(template: &str) -> Option<RootKind> {
    if template.starts_with("${project}") {
        Some(RootKind::Project)
    } else if template.starts_with("${plugin}") {
        Some(RootKind::Plugin)
    } else {
        None
    }
}

fn map_path_err(e: PathError) -> CompileError {
    match e {
        PathError::UnknownPlaceholder | PathError::MalformedPlaceholder => {
            CompileError::UnknownPlaceholder
        }
        PathError::PathEscape | PathError::NotAbsolute => CompileError::PathEscape,
        PathError::SymlinkEscape => CompileError::SymlinkEscape,
        PathError::Io(io) => CompileError::Io(io),
    }
}

fn most_permissive_mode(a: NetworkMode, b: NetworkMode) -> NetworkMode {
    fn rank(m: NetworkMode) -> u8 {
        match m {
            NetworkMode::Deny => 0,
            NetworkMode::Proxy => 1,
            NetworkMode::AllowAll => 2,
        }
    }
    if rank(b) > rank(a) {
        b
    } else {
        a
    }
}

fn max_opt(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.max(y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

fn spot_check_v3(
    lock: &Lock,
    canonical: &CanonicalId,
    entry: &PluginEntry,
) -> Result<(), CompileError> {
    let prefix_pairs: Vec<(CanonicalId, String)> = lock
        .plugins
        .keys()
        .map(|c| (c.clone(), topic_id::derive(&c.to_string())))
        .collect();
    if topic_id::collisions_with_prefixes(&prefix_pairs).is_err() {
        return Err(CompileError::ValidationNotRun);
    }

    let mut tool_claims: BTreeMap<&str, usize> = BTreeMap::new();
    for e in lock.plugins.values() {
        for tool in &e.bindings.tools {
            *tool_claims.entry(tool.as_str()).or_default() += 1;
        }
    }
    let resolved: BTreeSet<&str> = lock
        .session
        .tool_owner
        .keys()
        .map(String::as_str)
        .collect();
    for (tool, claims) in &tool_claims {
        if *claims > 1 && !resolved.contains(tool) {
            return Err(CompileError::ValidationNotRun);
        }
    }

    let own_topic_id = topic_id::derive(&canonical.to_string());
    for topic in &entry.grant.publishes {
        let mut segs = topic.split('.');
        let Some(first) = segs.next() else { continue };
        match first {
            "core" | "frontend" => return Err(CompileError::ValidationNotRun),
            "plugin" => {
                if let Some(second) = segs.next() {
                    if second != own_topic_id {
                        return Err(CompileError::ValidationNotRun);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}
