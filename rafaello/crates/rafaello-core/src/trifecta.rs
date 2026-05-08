//! Trifecta refusal evaluation per scope §Tr1–§Tr5.
//!
//! `evaluate` computes the three booleans `(reads_untrusted,
//! has_outbound, has_workspace_write)` plus `refuse` over the full
//! bundle union of the named plugin's lock entry. The private-state
//! subtree is structurally excluded from `has_workspace_write`: the
//! lock simply has no `write_dirs` entry for it (C5 injects that
//! grant later in the compiler, after trifecta runs).

use std::path::Path;

use crate::lock::{CanonicalId, Grant, GrantBundle, Lock};
use crate::manifest::capabilities::NetworkMode;
use crate::manifest::placeholders;
use crate::paths::PathContext;
use crate::validate::pattern_matches_topic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrifectaState {
    pub reads_untrusted: bool,
    pub has_outbound: bool,
    pub has_workspace_write: bool,
    pub refuse: bool,
}

pub fn evaluate(lock: &Lock, canonical: &CanonicalId, ctx: &PathContext) -> TrifectaState {
    let entry = match lock.plugins.get(canonical) {
        Some(e) => e,
        None => {
            return TrifectaState {
                reads_untrusted: false,
                has_outbound: false,
                has_workspace_write: false,
                refuse: false,
            };
        }
    };

    let bundles: Vec<&GrantBundle> = entry.grant.bundles.values().collect();

    let any_own_network_open = bundles.iter().any(|b| network_open(b));
    let reads_outside_project = bundles
        .iter()
        .any(|b| bundle_reads_outside_project(b, ctx));
    let subscribes_session_signal = entry.grant.subscribes.iter().any(|pat| {
        pattern_matches_topic(pat, "core.session.tool_result")
            || pattern_matches_topic(pat, "core.session.assistant_message")
    });
    let reads_untrusted =
        any_own_network_open || reads_outside_project || subscribes_session_signal;

    let has_outbound =
        any_own_network_open || other_plugin_one_hop_outbound(lock, canonical, &entry.grant);

    let has_workspace_write = bundles.iter().any(|b| {
        b.filesystem
            .as_ref()
            .is_some_and(|fs| !fs.write_dirs.is_empty())
    });

    let refuse = reads_untrusted
        && has_outbound
        && has_workspace_write
        && !entry.flags.i_know_what_im_doing;

    TrifectaState {
        reads_untrusted,
        has_outbound,
        has_workspace_write,
        refuse,
    }
}

fn network_open(bundle: &GrantBundle) -> bool {
    bundle
        .network
        .as_ref()
        .is_some_and(|n| n.mode != NetworkMode::Deny)
}

fn bundle_reads_outside_project(bundle: &GrantBundle, ctx: &PathContext) -> bool {
    let Some(fs) = &bundle.filesystem else {
        return false;
    };
    fs.read_dirs
        .iter()
        .chain(fs.read_paths.iter())
        .any(|template| match placeholders::expand(template, ctx) {
            Ok(expanded) => !Path::new(&expanded).starts_with(&ctx.project_root),
            Err(_) => false,
        })
}

fn other_plugin_one_hop_outbound(
    lock: &Lock,
    canonical: &CanonicalId,
    own_grant: &Grant,
) -> bool {
    if own_grant.publishes.is_empty() {
        return false;
    }
    for (other_id, other_entry) in &lock.plugins {
        if other_id == canonical {
            continue;
        }
        let other_open = other_entry.grant.bundles.values().any(network_open);
        if !other_open {
            continue;
        }
        let crosses = own_grant.publishes.iter().any(|topic| {
            other_entry
                .grant
                .subscribes
                .iter()
                .any(|pat| pattern_matches_topic(pat, topic))
        });
        if crosses {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    //! Tr4 sanity: a programmatic lock with no `write_dirs` entries
    //! reports `has_workspace_write == false`. The structural
    //! exclusion of the per-plugin private-state subtree is realised
    //! by the lock simply having no entry for it (C5 injects later);
    //! the integration test that exercises the compiler injection
    //! lands as `compile_private_state_excluded_from_workspace_write`
    //! in c31.
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use chrono::{TimeZone, Utc};

    use super::evaluate;
    use crate::lock::{
        Bindings, CanonicalId, Grant, GrantBundle, GrantFilesystem, Lock, LockFlags, PluginEntry,
    };
    use crate::manifest::safepath::SafePath;
    use crate::paths::PathContext;

    #[test]
    fn no_write_dirs_means_no_workspace_write() {
        let id = CanonicalId::parse("github.com/acme:reader@1.0.0").unwrap();
        let mut bundles = BTreeMap::new();
        bundles.insert(
            "default".to_owned(),
            GrantBundle {
                filesystem: Some(GrantFilesystem {
                    read_dirs: vec!["${project}/src".to_owned()],
                    ..GrantFilesystem::default()
                }),
                ..GrantBundle::default()
            },
        );
        let entry = PluginEntry {
            entry: SafePath::parse("bin/r.js").unwrap(),
            digest: "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_owned(),
            manifest_digest:
                "sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
                    .to_owned(),
            granted_at: Utc.with_ymd_and_hms(2026, 1, 15, 8, 30, 0).unwrap(),
            grant: Grant {
                bundles,
                ..Grant::default()
            },
            bindings: Bindings::default(),
            flags: LockFlags::default(),
        };
        let mut plugins = BTreeMap::new();
        plugins.insert(id.clone(), entry);
        let lock = Lock {
            plugins,
            session: Default::default(),
        };
        let ctx = PathContext {
            project_root: PathBuf::from("/work/proj"),
            home: PathBuf::from("/home/u"),
            plugin_dir: PathBuf::from("/work/proj/.rafaello/plugins/r"),
            cache_dir: PathBuf::from("/home/u/.cache/rafaello"),
            state_dir: PathBuf::from("/home/u/.local/state/rafaello"),
        };
        let state = evaluate(&lock, &id, &ctx);
        assert!(!state.has_workspace_write);
        assert!(!state.refuse);
    }
}
