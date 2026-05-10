//! Conversation entry model — `Entry`, metadata, and the eight built-in
//! payload kinds. See `rafaello/plans/streams/e-renderer/rfc-renderer-model.md`
//! for the schema this implements.
//!
//! The renderer pipeline is `Entry -> RenderNode -> frontend paint`; this
//! module owns the `Entry` half. `RenderNode` lives next door in
//! [`render_node`].

pub mod payloads;
pub mod render_node;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ulid::Ulid;

pub use payloads::ToolCallStatus;
pub use render_node::{RawFormat, RenderNode};

use payloads::{
    CodeBlockPayload, ErrorPayload, HeadingPayload, ImagePayload, TextPayload, ThinkingPayload,
    ToolCallPayload, ToolResultPayload,
};

/// Originator of an [`Entry`]. Encoded snake_case to match the RFC schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryAuthor {
    User,
    Assistant,
    Tool,
    System,
    Plugin,
}

/// Streaming lifecycle of an entry. v1 surfaces only `Final`; `open`,
/// `patch`, and `closed` arrive when streaming patches land.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamState {
    Final,
}

/// Plain-text fallback an author ships alongside a structured payload, so
/// frontends without the matching renderer can still display *something*.
/// See RFC §6.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryFallback {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// Per-entry metadata (timestamps, author, streaming state, tags).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryMetadata {
    pub created_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
    pub author: EntryAuthor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin: Option<String>,
    pub stream_state: StreamState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<u64>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// One conversation event. `kind` is the routing key; `payload` is opaque
/// JSON whose shape is determined by `kind` (see [`payloads`]).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub id: Ulid,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<Ulid>,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,
    pub payload: Value,
    pub metadata: EntryMetadata,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<EntryFallback>,
}

impl Entry {
    fn assemble(kind: &str, author: EntryAuthor, payload: Value) -> Self {
        Self {
            id: Ulid::new(),
            parent: None,
            kind: kind.to_string(),
            schema: None,
            payload,
            metadata: EntryMetadata {
                created_at: Utc::now(),
                updated_at: None,
                author,
                plugin: None,
                stream_state: StreamState::Final,
                seq: None,
                tags: Vec::new(),
            },
            fallback: None,
        }
    }

    pub fn new_text(text: &str) -> Self {
        let payload = TextPayload {
            text: text.to_string(),
            markdown: false,
        };
        Self::assemble(
            "text",
            EntryAuthor::Assistant,
            serde_json::to_value(payload).expect("TextPayload serializes"),
        )
    }

    pub fn new_heading(level: u8, text: &str) -> Self {
        let payload = HeadingPayload {
            text: text.to_string(),
            level,
        };
        Self::assemble(
            "heading",
            EntryAuthor::Assistant,
            serde_json::to_value(payload).expect("HeadingPayload serializes"),
        )
    }

    pub fn new_code_block(code: &str, lang: Option<&str>) -> Self {
        let payload = CodeBlockPayload {
            code: code.to_string(),
            lang: lang.map(|s| s.to_string()),
        };
        Self::assemble(
            "code_block",
            EntryAuthor::Assistant,
            serde_json::to_value(payload).expect("CodeBlockPayload serializes"),
        )
    }

    pub fn new_thinking(text: &str) -> Self {
        let payload = ThinkingPayload {
            text: text.to_string(),
        };
        Self::assemble(
            "thinking",
            EntryAuthor::Assistant,
            serde_json::to_value(payload).expect("ThinkingPayload serializes"),
        )
    }

    pub fn new_tool_call(id: &str, name: &str, args: Value, status: ToolCallStatus) -> Self {
        let payload = ToolCallPayload {
            id: id.to_string(),
            name: name.to_string(),
            args,
            status,
        };
        Self::assemble(
            "tool_call",
            EntryAuthor::Assistant,
            serde_json::to_value(payload).expect("ToolCallPayload serializes"),
        )
    }

    pub fn new_tool_result(call_id: &str, ok: bool, content: RenderNode) -> Self {
        let payload = ToolResultPayload {
            call_id: call_id.to_string(),
            ok,
            content,
            details: None,
        };
        Self::assemble(
            "tool_result",
            EntryAuthor::Tool,
            serde_json::to_value(payload).expect("ToolResultPayload serializes"),
        )
    }

    pub fn new_image(uri: &str, mime: &str, alt: &str) -> Self {
        let payload = ImagePayload {
            uri: uri.to_string(),
            mime: mime.to_string(),
            alt: alt.to_string(),
            bytes_b64: None,
        };
        Self::assemble(
            "image",
            EntryAuthor::Assistant,
            serde_json::to_value(payload).expect("ImagePayload serializes"),
        )
    }

    pub fn new_error(code: &str, message: &str) -> Self {
        let payload = ErrorPayload {
            code: code.to_string(),
            message: message.to_string(),
            data: None,
        };
        Self::assemble(
            "error",
            EntryAuthor::System,
            serde_json::to_value(payload).expect("ErrorPayload serializes"),
        )
    }

    pub fn new_unknown(kind: &str, payload: Value, fallback: EntryFallback) -> Self {
        let mut entry = Self::assemble(kind, EntryAuthor::Plugin, payload);
        entry.fallback = Some(fallback);
        entry
    }
}
