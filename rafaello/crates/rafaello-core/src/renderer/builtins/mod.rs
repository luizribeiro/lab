//! Built-in renderers for the four built-in entry kinds covered by Stream E
//! §3.1: `text`, `heading`, `code_block`, `thinking`. Each parses the typed
//! payload from `entry.payload` and projects it into a `RenderNode` per
//! Stream E §4.1.

use crate::entry::payloads::{
    CodeBlockPayload, ErrorPayload, HeadingPayload, ImagePayload, TextPayload, ThinkingPayload,
    ToolCallPayload, ToolCallStatus, ToolResultPayload,
};
use crate::entry::render_node::{CalloutKind, KeyValuePair};
use crate::entry::{Entry, RenderNode};

use super::{Capabilities, Renderer, RendererError};

fn status_str(status: &ToolCallStatus) -> &'static str {
    match status {
        ToolCallStatus::Pending => "pending",
        ToolCallStatus::Running => "running",
        ToolCallStatus::Ok => "ok",
        ToolCallStatus::Error => "error",
    }
}

fn text_node(text: impl Into<String>) -> RenderNode {
    RenderNode::Text {
        text: text.into(),
        emphasis: None,
    }
}

fn decode<T: serde::de::DeserializeOwned>(kind: &str, entry: &Entry) -> Result<T, RendererError> {
    serde_json::from_value(entry.payload.clone()).map_err(|e| RendererError::InvalidPayload {
        kind: kind.to_string(),
        message: e.to_string(),
    })
}

pub struct TextRenderer;

impl Renderer for TextRenderer {
    fn render(&self, entry: &Entry, _caps: &Capabilities) -> Result<RenderNode, RendererError> {
        let payload: TextPayload = decode("text", entry)?;
        Ok(RenderNode::Text {
            text: payload.text,
            emphasis: None,
        })
    }
}

pub struct HeadingRenderer;

impl Renderer for HeadingRenderer {
    fn render(&self, entry: &Entry, _caps: &Capabilities) -> Result<RenderNode, RendererError> {
        let payload: HeadingPayload = decode("heading", entry)?;
        Ok(RenderNode::Heading {
            level: payload.level,
            text: payload.text,
        })
    }
}

pub struct CodeBlockRenderer;

impl Renderer for CodeBlockRenderer {
    fn render(&self, entry: &Entry, _caps: &Capabilities) -> Result<RenderNode, RendererError> {
        let payload: CodeBlockPayload = decode("code_block", entry)?;
        Ok(RenderNode::Code {
            code: payload.code,
            lang: payload.lang,
        })
    }
}

pub struct ToolCallRenderer;

impl Renderer for ToolCallRenderer {
    fn render(&self, entry: &Entry, _caps: &Capabilities) -> Result<RenderNode, RendererError> {
        let payload: ToolCallPayload = decode("tool_call", entry)?;
        let args_pretty = serde_json::to_string_pretty(&payload.args).map_err(|e| {
            RendererError::InvalidPayload {
                kind: "tool_call".to_string(),
                message: e.to_string(),
            }
        })?;
        Ok(RenderNode::KeyValue {
            pairs: vec![
                KeyValuePair {
                    key: "name".into(),
                    value: text_node(payload.name),
                },
                KeyValuePair {
                    key: "args".into(),
                    value: text_node(args_pretty),
                },
                KeyValuePair {
                    key: "status".into(),
                    value: text_node(status_str(&payload.status)),
                },
            ],
        })
    }
}

pub struct ToolResultRenderer;

impl Renderer for ToolResultRenderer {
    fn render(&self, entry: &Entry, _caps: &Capabilities) -> Result<RenderNode, RendererError> {
        let payload: ToolResultPayload = decode("tool_result", entry)?;
        let header = RenderNode::Heading {
            level: 4,
            text: format!(
                "tool_result {} ({})",
                payload.call_id,
                if payload.ok { "ok" } else { "error" }
            ),
        };
        Ok(RenderNode::Block {
            children: vec![header, payload.content],
        })
    }
}

pub struct ImageRenderer;

impl Renderer for ImageRenderer {
    fn render(&self, entry: &Entry, _caps: &Capabilities) -> Result<RenderNode, RendererError> {
        let payload: ImagePayload = decode("image", entry)?;
        Ok(RenderNode::Image {
            uri: payload.uri,
            mime: payload.mime,
            alt: payload.alt,
            bytes_b64: None,
        })
    }
}

pub struct ErrorRenderer;

impl Renderer for ErrorRenderer {
    fn render(&self, entry: &Entry, _caps: &Capabilities) -> Result<RenderNode, RendererError> {
        let payload: ErrorPayload = decode("error", entry)?;
        Ok(RenderNode::Callout {
            kind: CalloutKind::Error,
            child: Box::new(RenderNode::KeyValue {
                pairs: vec![
                    KeyValuePair {
                        key: "code".into(),
                        value: text_node(payload.code),
                    },
                    KeyValuePair {
                        key: "message".into(),
                        value: text_node(payload.message),
                    },
                ],
            }),
        })
    }
}

pub struct ThinkingRenderer;

impl Renderer for ThinkingRenderer {
    fn render(&self, entry: &Entry, _caps: &Capabilities) -> Result<RenderNode, RendererError> {
        let payload: ThinkingPayload = decode("thinking", entry)?;
        Ok(RenderNode::Collapsed {
            summary: Box::new(RenderNode::Text {
                text: "thinking".into(),
                emphasis: None,
            }),
            detail: Box::new(RenderNode::Text {
                text: payload.text,
                emphasis: None,
            }),
            default_open: false,
        })
    }
}
