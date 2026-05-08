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

use crate::error::CompileError;
use crate::digest::RecomputedDigests;
use crate::lock::canonical_id::CanonicalId;
use crate::lock::load_policy::LoadPolicy;
use crate::lock::{
    Grant, GrantEnv, GrantFilesystem, GrantLimits, GrantNetwork, Lock, PluginEntry,
};
use crate::manifest::capabilities::NetworkMode;
use crate::paths::PathContext;
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
/// The body is a scaffold: per-section emitters land in c30–c34.
/// Today it returns a minimally populated [`CompiledPlugin`] with
/// `entry_absolute = ctx.plugin_dir.join(entry)` and default-empty
/// plans; later commits fill in the bundle-union, path resolver,
/// carve-out, network/env, digest gating, and tool_meta layers.
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

    let filesystem = FilesystemPlan {
        read_paths: eff.filesystem.read_paths.iter().map(PathBuf::from).collect(),
        read_dirs: eff.filesystem.read_dirs.iter().map(PathBuf::from).collect(),
        write_paths: eff.filesystem.write_paths.iter().map(PathBuf::from).collect(),
        write_dirs: eff.filesystem.write_dirs.iter().map(PathBuf::from).collect(),
        exec_paths: eff.filesystem.exec_paths.iter().map(PathBuf::from).collect(),
        exec_dirs: eff.filesystem.exec_dirs.iter().map(PathBuf::from).collect(),
    };

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
