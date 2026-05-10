//! `RenderPipeline` — turns an [`Entry`] into a [`RenderNode`] using a
//! [`RendererRegistry`] plus terminal [`Capabilities`].
//!
//! Implements scope §R3 (three paths):
//!   * Path A: unknown entry kind / renderer-unavailable → fallback Block or
//!     warn Callout.
//!   * Path B: registered renderer panics or returns `Err(_)` → log, fall
//!     into Path A.
//!   * Path C: capability-driven downgrade — walk the returned tree; replace
//!     any node the terminal can't paint with `RenderNode::Unknown`.

use std::panic::AssertUnwindSafe;
use std::sync::Arc;

use serde_json::Value;

use crate::entry::render_node::{CalloutKind, KeyValuePair, RawFormat};
use crate::entry::{Entry, RenderNode};

use super::{Capabilities, RendererRegistry};

pub struct RenderPipeline {
    registry: Arc<RendererRegistry>,
}

impl RenderPipeline {
    pub fn new(registry: Arc<RendererRegistry>) -> Self {
        Self { registry }
    }

    pub fn render(&self, entry: &Entry, caps: &Capabilities) -> RenderNode {
        let Some(renderer) = self.registry.get(&entry.kind).cloned() else {
            return path_a(entry);
        };

        let result = std::panic::catch_unwind(AssertUnwindSafe(|| renderer.render(entry, caps)));
        match result {
            Err(_) => {
                tracing::error!(
                    kind = %entry.kind,
                    "renderer panicked; falling back to Path A"
                );
                path_a(entry)
            }
            Ok(Err(err)) => {
                tracing::warn!(
                    kind = %entry.kind,
                    error = %err,
                    "renderer returned Err; falling back to Path A"
                );
                path_a(entry)
            }
            Ok(Ok(tree)) => downgrade(tree, entry, caps),
        }
    }
}

fn path_a(entry: &Entry) -> RenderNode {
    if let Some(fb) = &entry.fallback {
        return RenderNode::Block {
            children: vec![RenderNode::Text {
                text: fb.text.clone(),
                emphasis: None,
            }],
        };
    }

    let payload_str = serde_json::to_string(&entry.payload).unwrap_or_default();
    RenderNode::Callout {
        kind: CalloutKind::Warn,
        child: Box::new(RenderNode::KeyValue {
            pairs: vec![
                KeyValuePair {
                    key: "kind".into(),
                    value: text(&entry.kind),
                },
                KeyValuePair {
                    key: "schema".into(),
                    value: text(entry.schema.as_deref().unwrap_or("")),
                },
                KeyValuePair {
                    key: "payload".into(),
                    value: text(&payload_str),
                },
            ],
        }),
    }
}

fn text(s: &str) -> RenderNode {
    RenderNode::Text {
        text: s.to_string(),
        emphasis: None,
    }
}

fn downgrade(node: RenderNode, entry: &Entry, caps: &Capabilities) -> RenderNode {
    if !node_supported(&node, caps) {
        return to_unknown(node, entry);
    }
    match node {
        RenderNode::Inline { children } => RenderNode::Inline {
            children: children
                .into_iter()
                .map(|c| downgrade(c, entry, caps))
                .collect(),
        },
        RenderNode::Block { children } => RenderNode::Block {
            children: children
                .into_iter()
                .map(|c| downgrade(c, entry, caps))
                .collect(),
        },
        RenderNode::List { ordered, items } => RenderNode::List {
            ordered,
            items: items
                .into_iter()
                .map(|c| downgrade(c, entry, caps))
                .collect(),
        },
        RenderNode::KeyValue { pairs } => RenderNode::KeyValue {
            pairs: pairs
                .into_iter()
                .map(|p| KeyValuePair {
                    key: p.key,
                    value: downgrade(p.value, entry, caps),
                })
                .collect(),
        },
        RenderNode::Table { headers, rows } => RenderNode::Table {
            headers,
            rows: rows
                .into_iter()
                .map(|row| row.into_iter().map(|c| downgrade(c, entry, caps)).collect())
                .collect(),
        },
        RenderNode::Link { href, child } => RenderNode::Link {
            href,
            child: Box::new(downgrade(*child, entry, caps)),
        },
        RenderNode::Callout { kind, child } => RenderNode::Callout {
            kind,
            child: Box::new(downgrade(*child, entry, caps)),
        },
        RenderNode::Collapsed {
            summary,
            detail,
            default_open,
        } => RenderNode::Collapsed {
            summary: Box::new(downgrade(*summary, entry, caps)),
            detail: Box::new(downgrade(*detail, entry, caps)),
            default_open,
        },
        leaf => leaf,
    }
}

fn node_supported(node: &RenderNode, caps: &Capabilities) -> bool {
    if !caps.nodes.contains(node_name(node)) {
        return false;
    }
    if let RenderNode::Raw { format, .. } = node {
        if !caps.raw_formats.contains(raw_format_name(format)) {
            return false;
        }
    }
    true
}

fn to_unknown(node: RenderNode, entry: &Entry) -> RenderNode {
    let kind = node_name(&node).to_string();
    let payload = serde_json::to_value(&node).unwrap_or(Value::Null);
    RenderNode::Unknown {
        kind,
        payload,
        fallback: entry.fallback.clone().unwrap_or_default(),
    }
}

fn node_name(node: &RenderNode) -> &'static str {
    match node {
        RenderNode::Text { .. } => "Text",
        RenderNode::Heading { .. } => "Heading",
        RenderNode::Code { .. } => "Code",
        RenderNode::Inline { .. } => "Inline",
        RenderNode::Block { .. } => "Block",
        RenderNode::List { .. } => "List",
        RenderNode::KeyValue { .. } => "KeyValue",
        RenderNode::Table { .. } => "Table",
        RenderNode::Divider {} => "Divider",
        RenderNode::Image { .. } => "Image",
        RenderNode::Link { .. } => "Link",
        RenderNode::Callout { .. } => "Callout",
        RenderNode::Collapsed { .. } => "Collapsed",
        RenderNode::Raw { .. } => "Raw",
        RenderNode::Unknown { .. } => "Unknown",
    }
}

fn raw_format_name(f: &RawFormat) -> &'static str {
    match f {
        RawFormat::Ansi => "ansi",
        RawFormat::Html => "html",
        RawFormat::Plain => "plain",
    }
}
