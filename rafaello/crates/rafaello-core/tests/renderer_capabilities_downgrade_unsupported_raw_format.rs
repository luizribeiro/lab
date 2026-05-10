//! Path C — `RenderNode::Raw` whose format is not in `caps.raw_formats` must
//! be downgraded to `RenderNode::Unknown`, even though `Raw` itself is in
//! `caps.nodes`.

use std::sync::Arc;

use rafaello_core::entry::render_node::RawFormat;
use rafaello_core::entry::{Entry, EntryFallback, RenderNode};
use rafaello_core::{Capabilities, RenderPipeline, Renderer, RendererError, RendererRegistry};

struct HtmlRawRenderer;

impl Renderer for HtmlRawRenderer {
    fn render(&self, _entry: &Entry, _caps: &Capabilities) -> Result<RenderNode, RendererError> {
        Ok(RenderNode::Raw {
            format: RawFormat::Html,
            body: "<b>hi</b>".into(),
        })
    }
}

#[test]
fn raw_with_unsupported_format_is_downgraded_to_unknown() {
    let mut registry = RendererRegistry::new();
    registry.register("test:raw-html".into(), Arc::new(HtmlRawRenderer));
    let pipeline = RenderPipeline::new(Arc::new(registry));

    let caps = Capabilities::tui_default();
    assert!(caps.nodes.contains("Raw"));
    assert!(!caps.raw_formats.contains("html"));

    let mut entry = Entry::new_text("ignored");
    entry.kind = "test:raw-html".into();
    entry.fallback = Some(EntryFallback {
        text: "html not paintable here".into(),
        markdown: None,
        summary: None,
    });

    let out = pipeline.render(&entry, &caps);

    match out {
        RenderNode::Unknown {
            kind,
            payload,
            fallback,
        } => {
            assert_eq!(kind, "Raw");
            assert_eq!(payload["node"], serde_json::json!("Raw"));
            assert_eq!(payload["format"], serde_json::json!("html"));
            assert_eq!(payload["body"], serde_json::json!("<b>hi</b>"));
            assert_eq!(fallback.text, "html not paintable here");
        }
        other => panic!("expected Unknown, got {other:?}"),
    }
}

#[test]
fn raw_with_supported_format_passes_through() {
    let mut registry = RendererRegistry::new();
    struct AnsiRaw;
    impl Renderer for AnsiRaw {
        fn render(
            &self,
            _entry: &Entry,
            _caps: &Capabilities,
        ) -> Result<RenderNode, RendererError> {
            Ok(RenderNode::Raw {
                format: RawFormat::Ansi,
                body: "\x1b[1mok\x1b[0m".into(),
            })
        }
    }
    registry.register("test:raw-ansi".into(), Arc::new(AnsiRaw));
    let pipeline = RenderPipeline::new(Arc::new(registry));

    let mut entry = Entry::new_text("ignored");
    entry.kind = "test:raw-ansi".into();

    let out = pipeline.render(&entry, &Capabilities::tui_default());
    match out {
        RenderNode::Raw {
            format: RawFormat::Ansi,
            body,
        } => assert!(body.contains("ok")),
        other => panic!("expected Raw passthrough, got {other:?}"),
    }
}
