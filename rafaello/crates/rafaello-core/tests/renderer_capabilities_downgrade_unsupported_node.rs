//! Path C — renderer returns Ok(tree) but the terminal lacks support for one
//! of the node kinds. The pipeline walks the tree and replaces the
//! unsupported subtree with `RenderNode::Unknown` carrying the entry's
//! fallback.

use std::sync::Arc;

use rafaello_core::entry::render_node::KeyValuePair;
use rafaello_core::entry::{Entry, EntryFallback, RenderNode};
use rafaello_core::{Capabilities, RenderPipeline, Renderer, RendererError, RendererRegistry};

struct TableRenderer;

impl Renderer for TableRenderer {
    fn render(&self, _entry: &Entry, _caps: &Capabilities) -> Result<RenderNode, RendererError> {
        Ok(RenderNode::Block {
            children: vec![
                RenderNode::Text {
                    text: "before".into(),
                    emphasis: None,
                },
                RenderNode::Table {
                    headers: vec!["a".into()],
                    rows: vec![vec![RenderNode::Text {
                        text: "row".into(),
                        emphasis: None,
                    }]],
                },
                RenderNode::KeyValue {
                    pairs: vec![KeyValuePair {
                        key: "k".into(),
                        value: RenderNode::Text {
                            text: "v".into(),
                            emphasis: None,
                        },
                    }],
                },
            ],
        })
    }
}

#[test]
fn unsupported_node_in_tree_is_replaced_with_unknown_carrying_fallback() {
    let mut registry = RendererRegistry::new();
    registry.register("test:table".into(), Arc::new(TableRenderer));
    let pipeline = RenderPipeline::new(Arc::new(registry));

    let mut caps = Capabilities::tui_default();
    caps.nodes.remove("Table");

    let mut entry = Entry::new_text("ignored");
    entry.kind = "test:table".into();
    entry.fallback = Some(EntryFallback {
        text: "table not supported".into(),
        markdown: None,
        summary: None,
    });

    let out = pipeline.render(&entry, &caps);

    let RenderNode::Block { children } = out else {
        panic!("expected outer Block preserved");
    };
    assert_eq!(children.len(), 3);
    assert!(matches!(children[0], RenderNode::Text { .. }));

    match &children[1] {
        RenderNode::Unknown {
            kind,
            payload,
            fallback,
        } => {
            assert_eq!(kind, "Table");
            assert_eq!(payload["node"], serde_json::json!("Table"));
            assert_eq!(fallback.text, "table not supported");
        }
        other => panic!("expected Unknown replacement for Table, got {other:?}"),
    }

    assert!(matches!(children[2], RenderNode::KeyValue { .. }));
}
