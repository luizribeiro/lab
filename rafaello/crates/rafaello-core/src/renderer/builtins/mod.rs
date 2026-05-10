//! Built-in renderers for the four built-in entry kinds covered by Stream E
//! §3.1: `text`, `heading`, `code_block`, `thinking`. Each parses the typed
//! payload from `entry.payload` and projects it into a `RenderNode` per
//! Stream E §4.1.

use crate::entry::payloads::{CodeBlockPayload, HeadingPayload, TextPayload, ThinkingPayload};
use crate::entry::{Entry, RenderNode};

use super::{Capabilities, Renderer, RendererError};

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
