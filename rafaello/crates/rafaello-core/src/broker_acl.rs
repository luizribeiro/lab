//! Per-session broker ACL extraction per scope §G1–§G3.
//!
//! `compile(lock)` builds the typed value the m2 broker consumes:
//! per-plugin publish/subscribe authority + the compiler-inserted
//! `plugin.<topic-id>.tool_request` self-subscribe + the
//! compiler-inserted `plugin.<topic-id>.tool_result` auto-publish
//! for tool plugins (scope §M1.3) + the resolved tool-name routing
//! table (pi review-2 finding 4).
//!
//! Same V3-must-run-first contract as `compile::compile_plugin`:
//! a handful of obvious invariants are spot-checked and the
//! function returns [`CompileError::ValidationNotRun`] on a caught
//! violation rather than re-running V3. §G2's grammar
//! revalidation runs against every publish topic and every
//! subscribe pattern before emit.

use std::collections::{BTreeMap, BTreeSet};

use crate::compile::spot_check_v3;
use crate::error::{AttachIdParseError, CompileError};
use crate::lock::canonical_id::CanonicalId;
use crate::lock::Lock;
use crate::topic_id;
use crate::validate::topic::{validate_pattern, validate_topic};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BrokerAcl {
    pub plugins: BTreeMap<CanonicalId, PluginAcl>,
    pub tool_routes: BTreeMap<String, CanonicalId>,
    pub frontends: BTreeMap<AttachId, FrontendAcl>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AttachId(String);

impl AttachId {
    pub fn new(input: impl Into<String>) -> Result<Self, AttachIdParseError> {
        let s = input.into();
        let bytes = s.as_bytes();
        if bytes.is_empty() {
            return Err(AttachIdParseError::Empty);
        }
        if bytes.len() > 32 {
            return Err(AttachIdParseError::TooLong { len: bytes.len() });
        }
        let first = bytes[0];
        if !first.is_ascii_lowercase() {
            return Err(AttachIdParseError::IllegalLeadChar { ch: first as char });
        }
        for &b in &bytes[1..] {
            let ok = b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-';
            if !ok {
                return Err(AttachIdParseError::IllegalChar { ch: b as char });
            }
        }
        Ok(AttachId(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AttachId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FrontendAcl {
    pub subscribe_patterns: BTreeSet<String>,
    pub auto_subscribes: BTreeSet<String>,
    pub publish_topics: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginAcl {
    pub topic_id: String,
    pub publish_topics: Vec<String>,
    pub subscribe_patterns: Vec<String>,
    pub auto_subscribes: Vec<String>,
    pub provider_id: Option<String>,
}

pub fn compile(lock: &Lock) -> Result<BrokerAcl, CompileError> {
    let mut plugins: BTreeMap<CanonicalId, PluginAcl> = BTreeMap::new();

    for (canonical, entry) in &lock.plugins {
        spot_check_v3(lock, canonical, entry)?;

        for topic in &entry.grant.publishes {
            validate_topic(topic).map_err(|_| CompileError::ValidationNotRun)?;
        }
        for pattern in &entry.grant.subscribes {
            validate_pattern(pattern).map_err(|_| CompileError::ValidationNotRun)?;
        }

        let topic_id_str = topic_id::derive(&canonical.to_string());
        let auto_subscribes = vec![format!("plugin.{}.tool_request", topic_id_str)];
        let mut publish_topics = entry.grant.publishes.clone();
        if !entry.bindings.tools.is_empty() {
            publish_topics.push(format!("plugin.{}.tool_result", topic_id_str));
        }
        publish_topics.sort();
        publish_topics.dedup();
        let provider_id = if entry.bindings.provider {
            entry.bindings.provider_id.clone()
        } else {
            None
        };

        plugins.insert(
            canonical.clone(),
            PluginAcl {
                topic_id: topic_id_str,
                publish_topics,
                subscribe_patterns: entry.grant.subscribes.clone(),
                auto_subscribes,
                provider_id,
            },
        );
    }

    let mut declarers: BTreeMap<String, Vec<&CanonicalId>> = BTreeMap::new();
    for (canonical, entry) in &lock.plugins {
        for tool in &entry.bindings.tools {
            declarers.entry(tool.clone()).or_default().push(canonical);
        }
    }

    let mut tool_routes: BTreeMap<String, CanonicalId> = BTreeMap::new();
    for (tool, ds) in &declarers {
        let owner = if ds.len() == 1 {
            ds[0].clone()
        } else {
            let owner_str = lock
                .session
                .tool_owner
                .get(tool)
                .ok_or(CompileError::ValidationNotRun)?;
            CanonicalId::parse(owner_str).map_err(|_| CompileError::ValidationNotRun)?
        };
        tool_routes.insert(tool.clone(), owner);
    }

    Ok(BrokerAcl {
        plugins,
        tool_routes,
        frontends: BTreeMap::new(),
    })
}
