//! `RenderNode` ADT — the semantic render tree shipped from renderers to
//! frontends. See `rafaello/plans/streams/e-renderer/rfc-renderer-model.md`
//! §4 for the spec.

use serde::{Deserialize, Serialize};

use super::EntryFallback;

/// Semantic emphasis carried by inline [`RenderNode::Text`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Emphasis {
    None,
    Em,
    Strong,
    Dim,
    Warn,
    Err,
}

/// Severity-style for a [`RenderNode::Callout`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CalloutKind {
    Info,
    Warn,
    Error,
    Success,
}

/// Format of an embedded [`RenderNode::Raw`] body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RawFormat {
    Ansi,
    Html,
    Plain,
}

/// One key/value entry inside [`RenderNode::KeyValue`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyValuePair {
    pub key: String,
    pub value: RenderNode,
}

/// The 15 v1 render-tree variants. Internally tagged on `node` per RFC §4.2.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "node")]
pub enum RenderNode {
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        emphasis: Option<Emphasis>,
    },
    Heading {
        level: u8,
        text: String,
    },
    Code {
        code: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        lang: Option<String>,
    },
    Inline {
        children: Vec<RenderNode>,
    },
    Block {
        children: Vec<RenderNode>,
    },
    List {
        ordered: bool,
        items: Vec<RenderNode>,
    },
    KeyValue {
        pairs: Vec<KeyValuePair>,
    },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<RenderNode>>,
    },
    Divider {},
    Image {
        uri: String,
        mime: String,
        alt: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bytes_b64: Option<String>,
    },
    Link {
        href: String,
        child: Box<RenderNode>,
    },
    Callout {
        kind: CalloutKind,
        child: Box<RenderNode>,
    },
    Collapsed {
        summary: Box<RenderNode>,
        detail: Box<RenderNode>,
        default_open: bool,
    },
    Raw {
        format: RawFormat,
        body: String,
    },
    Unknown {
        kind: String,
        payload: serde_json::Value,
        fallback: EntryFallback,
    },
}
