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
use crate::lock::{Lock, PluginEntry};
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

    Ok(CompiledPlugin {
        canonical: canonical.clone(),
        topic_id,
        entry_absolute,
        filesystem: FilesystemPlan::default(),
        network: NetworkPlan::default(),
        env: EnvPlan::default(),
        limits: LimitsPlan::default(),
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
