//! Single-plugin and lock-level validation (scope §V).
//!
//! V1 lands here: `manifest_standalone(&Manifest) -> Result<(),
//! ValidationError>` performs every check that the parse commits
//! deferred per the m1-manifest phase boundary. V2 (canonical-id-
//! aware) and V3 (lock-level) land in later commits.

pub mod topic;

pub use topic::{
    is_custom_sink_class, is_tool_name, is_topic_segment, is_vendor_or_kind_part,
    pattern_matches_topic, validate_pattern, validate_topic,
};

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::error::ValidationError;
use crate::lock::{CanonicalId, Lock};
use crate::manifest::{Bus, Capabilities, Load, Manifest, NetworkMode, Provides, Renderer};
use crate::topic_id;

#[derive(Debug, Clone)]
pub struct LockValidationContext {
    pub project_root: PathBuf,
    pub home: PathBuf,
    pub plugin_dirs: BTreeMap<CanonicalId, PathBuf>,
    pub cache_root: PathBuf,
    pub state_root: PathBuf,
}

const KNOWN_SINK_CLASSES: &[&str] = &["network", "vcs_push", "mail", "workspace_write", "exec"];

const BUILTIN_RENDERER_KINDS: &[&str] = &[
    "text",
    "code_block",
    "tool_call",
    "tool_result",
    "error",
    "heading",
    "thinking",
    "image",
];

pub fn manifest_standalone(manifest: &Manifest) -> Result<(), ValidationError> {
    check_manifest_name(&manifest.name)?;
    check_provides(manifest.provides.as_ref())?;
    check_capabilities(manifest.capabilities.as_ref(), manifest.provides.as_ref())?;
    check_bus(manifest.bus.as_ref())?;
    check_renderers(&manifest.renderers)?;
    check_load(
        manifest.load.as_ref(),
        manifest.provides.as_ref(),
        manifest.bus.as_ref(),
        &manifest.renderers,
    )?;
    Ok(())
}

pub fn manifest_with_id(
    manifest: &Manifest,
    canonical: &CanonicalId,
) -> Result<(), ValidationError> {
    let own_topic_id = topic_id::derive(&canonical.to_string());
    let provider = manifest
        .provides
        .as_ref()
        .and_then(|p| p.provider.as_deref());
    let Some(bus) = manifest.bus.as_ref() else {
        return Ok(());
    };
    for topic in &bus.publishes {
        let mut segs = topic.split('.');
        let Some(first) = segs.next() else {
            continue;
        };
        let Some(second) = segs.next() else {
            continue;
        };
        match first {
            "plugin" if second != own_topic_id => {
                return Err(ValidationError::PublishOnForeignTopicId);
            }
            "provider" if provider != Some(second) => {
                return Err(ValidationError::ProviderNamespaceMismatch);
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn lock(lock: &Lock, ctx: &LockValidationContext) -> Result<(), ValidationError> {
    for canonical in lock.plugins.keys() {
        if !ctx.plugin_dirs.contains_key(canonical) {
            return Err(ValidationError::MissingPluginDir);
        }
    }

    let prefix_pairs: Vec<(CanonicalId, String)> = lock
        .plugins
        .keys()
        .map(|c| (c.clone(), topic_id::derive(&c.to_string())))
        .collect();
    topic_id::collisions_with_prefixes(&prefix_pairs)?;

    let mut tool_claims: BTreeMap<&str, Vec<&CanonicalId>> = BTreeMap::new();
    for (canonical, entry) in &lock.plugins {
        for tool in &entry.bindings.tools {
            tool_claims.entry(tool.as_str()).or_default().push(canonical);
        }
    }

    for (tool_name, owner_str) in &lock.session.tool_owner {
        let owner_id = CanonicalId::parse(owner_str)
            .map_err(|_| ValidationError::ToolOwnerUnknownPlugin)?;
        let Some(owner_entry) = lock.plugins.get(&owner_id) else {
            return Err(ValidationError::ToolOwnerUnknownPlugin);
        };
        if !owner_entry.bindings.tools.iter().any(|t| t == tool_name) {
            return Err(ValidationError::ToolOwnerPluginDoesNotDeclareTool);
        }
        let claim_count = tool_claims
            .get(tool_name.as_str())
            .map(|v| v.len())
            .unwrap_or(0);
        if claim_count <= 1 {
            return Err(ValidationError::ToolOwnerRedundant);
        }
    }

    for (tool_name, claimants) in &tool_claims {
        if claimants.len() > 1 && !lock.session.tool_owner.contains_key(*tool_name) {
            return Err(ValidationError::ConflictingToolName);
        }
    }

    if let Some(active_str) = &lock.session.provider_active {
        let active_id = CanonicalId::parse(active_str)
            .map_err(|_| ValidationError::ProviderActiveUnknown)?;
        let Some(active_entry) = lock.plugins.get(&active_id) else {
            return Err(ValidationError::ProviderActiveUnknown);
        };
        if !active_entry.bindings.provider || active_entry.bindings.provider_id.is_none() {
            return Err(ValidationError::ProviderActiveNotProvider);
        }
    }

    Ok(())
}

fn check_manifest_name(name: &str) -> Result<(), ValidationError> {
    if !is_tool_name(name) {
        return Err(ValidationError::IllegalManifestName {
            name: name.to_string(),
        });
    }
    Ok(())
}

fn check_provides(provides: Option<&Provides>) -> Result<(), ValidationError> {
    let Some(p) = provides else {
        return Ok(());
    };
    for tool in &p.tools {
        if !is_tool_name(tool) {
            return Err(ValidationError::IllegalToolName { name: tool.clone() });
        }
    }
    if let Some(provider) = &p.provider {
        if !is_tool_name(provider) {
            return Err(ValidationError::IllegalToolName {
                name: provider.clone(),
            });
        }
    }
    let declared: BTreeSet<&str> = p.tools.iter().map(String::as_str).collect();
    for (name, meta) in &p.tool {
        if !declared.contains(name.as_str()) {
            return Err(ValidationError::UnknownToolTable { tool: name.clone() });
        }
        if let Some(sinks) = &meta.sinks {
            for sink in sinks {
                if !is_valid_sink_class(sink) {
                    return Err(ValidationError::IllegalSinkClass {
                        class: sink.clone(),
                    });
                }
            }
        }
    }
    Ok(())
}

fn is_valid_sink_class(s: &str) -> bool {
    KNOWN_SINK_CLASSES.contains(&s) || is_custom_sink_class(s)
}

fn check_capabilities(
    capabilities: Option<&Capabilities>,
    provides: Option<&Provides>,
) -> Result<(), ValidationError> {
    let Some(caps) = capabilities else {
        return Ok(());
    };
    let tools: BTreeSet<&str> = provides
        .map(|p| p.tools.iter().map(String::as_str).collect())
        .unwrap_or_default();
    for (key, bundle) in caps {
        if key != "default" && !tools.contains(key.as_str()) {
            return Err(ValidationError::UnknownBundleKey {
                bundle: key.clone(),
            });
        }
        if let Some(net) = &bundle.network {
            if !net.allow_hosts.is_empty() && net.mode != NetworkMode::Proxy {
                return Err(ValidationError::AllowHostsOutsideProxy {
                    bundle: key.clone(),
                });
            }
        }
    }
    Ok(())
}

fn check_bus(bus: Option<&Bus>) -> Result<(), ValidationError> {
    let Some(b) = bus else {
        return Ok(());
    };
    for topic in &b.publishes {
        check_publish_topic(topic)?;
    }
    for pattern in &b.subscribes {
        validate_pattern(pattern)?;
    }
    Ok(())
}

fn check_publish_topic(topic: &str) -> Result<(), ValidationError> {
    if topic.split('.').any(|s| s == "*" || s == "**") {
        return Err(ValidationError::PatternInPublishPosition {
            topic: topic.to_string(),
        });
    }
    validate_topic(topic)?;
    let first = topic.split('.').next().unwrap_or("");
    match first {
        "core" => Err(ValidationError::PublishOnReservedNamespace {
            topic: topic.to_string(),
        }),
        "frontend" => Err(ValidationError::PublishOnFrontendNamespace {
            topic: topic.to_string(),
        }),
        _ => Ok(()),
    }
}

fn check_renderers(renderers: &[Renderer]) -> Result<(), ValidationError> {
    for r in renderers {
        check_renderer_kind(&r.kind)?;
    }
    Ok(())
}

fn check_renderer_kind(kind: &str) -> Result<(), ValidationError> {
    if BUILTIN_RENDERER_KINDS.contains(&kind) {
        return Err(ValidationError::ReservedRendererKind {
            kind: kind.to_string(),
        });
    }
    let Some((vendor, name)) = kind.split_once(':') else {
        return Err(ValidationError::UnprefixedRendererKind {
            kind: kind.to_string(),
        });
    };
    if !is_vendor_or_kind_part(vendor) || !is_vendor_or_kind_part(name) {
        return Err(ValidationError::UnprefixedRendererKind {
            kind: kind.to_string(),
        });
    }
    Ok(())
}

fn check_load(
    load: Option<&Load>,
    provides: Option<&Provides>,
    bus: Option<&Bus>,
    renderers: &[Renderer],
) -> Result<(), ValidationError> {
    let Some(Load::Lazy {
        event,
        command,
        kind,
    }) = load
    else {
        return Ok(());
    };
    let tools: BTreeSet<&str> = provides
        .map(|p| p.tools.iter().map(String::as_str).collect())
        .unwrap_or_default();
    for cmd in command {
        if !tools.contains(cmd.as_str()) {
            return Err(ValidationError::LoadTriggerUnknownCommand {
                command: cmd.clone(),
            });
        }
    }
    let subscribes: &[String] = bus.map(|b| b.subscribes.as_slice()).unwrap_or(&[]);
    for ev in event {
        validate_topic(ev)?;
        let matched = subscribes.iter().any(|p| pattern_matches_topic(p, ev));
        if !matched {
            return Err(ValidationError::LoadTriggerUnmatchedEvent { event: ev.clone() });
        }
    }
    let kinds: BTreeSet<&str> = renderers.iter().map(|r| r.kind.as_str()).collect();
    for k in kind {
        if !kinds.contains(k.as_str()) {
            return Err(ValidationError::LoadTriggerUnknownKind { kind: k.clone() });
        }
    }
    Ok(())
}
