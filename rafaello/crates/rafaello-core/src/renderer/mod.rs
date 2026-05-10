use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;

use thiserror::Error;

use crate::entry::{Entry, RenderNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnicodeClass {
    Ascii,
    Bmp,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorClass {
    None,
    Ansi16,
    Ansi256,
    Truecolor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollbackClass {
    None,
    AppendOnly,
    Mutable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capabilities {
    pub unicode: UnicodeClass,
    pub color: ColorClass,
    pub width: u32,
    pub height: Option<u32>,
    pub image: Vec<String>,
    pub interactive: bool,
    pub scrollback: ScrollbackClass,
    pub nodes: BTreeSet<String>,
    pub raw_formats: BTreeSet<String>,
}

impl Capabilities {
    pub fn tui_default() -> Self {
        let nodes: BTreeSet<String> = [
            "Text",
            "Heading",
            "Code",
            "Inline",
            "Block",
            "List",
            "KeyValue",
            "Table",
            "Divider",
            "Image",
            "Link",
            "Callout",
            "Collapsed",
            "Raw",
            "Unknown",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let raw_formats: BTreeSet<String> =
            ["ansi", "plain"].into_iter().map(String::from).collect();
        Self {
            unicode: UnicodeClass::Full,
            color: ColorClass::Truecolor,
            width: 120,
            height: None,
            image: Vec::new(),
            interactive: true,
            scrollback: ScrollbackClass::AppendOnly,
            nodes,
            raw_formats,
        }
    }
}

#[derive(Debug, Error)]
pub enum RendererError {
    #[error("renderer {kind}: missing payload field `{field}`")]
    MissingPayloadField { kind: String, field: String },
    #[error("renderer {kind}: invalid payload: {message}")]
    InvalidPayload { kind: String, message: String },
    #[error("renderer internal error: {detail}")]
    Internal { detail: String },
}

pub trait Renderer: Send + Sync + 'static {
    fn render(&self, entry: &Entry, caps: &Capabilities) -> Result<RenderNode, RendererError>;
}

#[derive(Default)]
pub struct RendererRegistry {
    renderers: BTreeMap<String, Arc<dyn Renderer>>,
}

impl RendererRegistry {
    pub fn new() -> Self {
        Self {
            renderers: BTreeMap::new(),
        }
    }

    pub fn with_builtins() -> Self {
        Self::new()
    }

    pub fn register(
        &mut self,
        kind: String,
        renderer: Arc<dyn Renderer>,
    ) -> Option<Arc<dyn Renderer>> {
        self.renderers.insert(kind, renderer)
    }

    pub fn get(&self, kind: &str) -> Option<&Arc<dyn Renderer>> {
        self.renderers.get(kind)
    }
}
