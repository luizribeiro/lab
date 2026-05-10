//! Paint layer — translates a [`RenderNode`] into ratatui widgets and
//! isolates panics from the redraw loop (scope §T4 + §T5).
//!
//! Production code calls [`draw_with_panic_isolation`]. Any panic raised
//! while painting is caught via [`std::panic::catch_unwind`] and
//! rendered as a `[render error: ...]` line on the terminal so the TUI
//! process keeps running.

use std::panic::AssertUnwindSafe;

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Terminal;

use rafaello_core::{PaintError, RenderNode};

/// Draws `node` onto `term`, catching any panic raised inside the paint
/// function and replacing the offending entry with a `[render error: ...]`
/// line. The TUI process never exits on a paint panic.
pub fn draw_with_panic_isolation<B: Backend>(
    term: &mut Terminal<B>,
    node: &RenderNode,
) -> Result<(), PaintError> {
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        term.draw(|frame| {
            let area = frame.area();
            paint_node::<B>(frame, area, node);
        })
        .map(|_| ())
    }));

    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(io)) => Err(PaintError::Draw(io)),
        Err(panic) => {
            let msg = panic_message(&panic);
            draw_error_line(term, &msg)
        }
    }
}

fn panic_message(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

fn draw_error_line<B: Backend>(term: &mut Terminal<B>, msg: &str) -> Result<(), PaintError> {
    let line = format!("[render error: {msg}]");
    term.draw(|frame| {
        let area = frame.area();
        frame.render_widget(Paragraph::new(line.clone()), area);
    })
    .map(|_| ())
    .map_err(PaintError::Draw)
}

fn paint_node<B: Backend>(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    node: &RenderNode,
) {
    match node {
        RenderNode::Text { text, .. } => {
            frame.render_widget(Paragraph::new(text.clone()), area);
        }
        RenderNode::Heading { text, .. } => {
            let span = Span::styled(text.clone(), Style::default().add_modifier(Modifier::BOLD));
            frame.render_widget(Paragraph::new(Line::from(span)), area);
        }
        RenderNode::Code { code, .. } => {
            frame.render_widget(Paragraph::new(code.clone()), area);
        }
        RenderNode::Block { children } | RenderNode::Inline { children } => {
            paint_children::<B>(frame, area, children);
        }
        RenderNode::List { items, .. } => {
            paint_children::<B>(frame, area, items);
        }
        RenderNode::KeyValue { pairs } => {
            let lines: Vec<Line> = pairs
                .iter()
                .map(|p| Line::from(format!("{}: {}", p.key, render_inline(&p.value))))
                .collect();
            frame.render_widget(Paragraph::new(lines), area);
        }
        RenderNode::Table { headers, rows } => {
            let mut lines: Vec<Line> = Vec::with_capacity(rows.len() + 1);
            lines.push(Line::from(headers.join(" | ")));
            for row in rows {
                let cells: Vec<String> = row.iter().map(render_inline).collect();
                lines.push(Line::from(cells.join(" | ")));
            }
            frame.render_widget(Paragraph::new(lines), area);
        }
        RenderNode::Divider {} => {
            frame.render_widget(Paragraph::new("─".repeat(area.width as usize)), area);
        }
        RenderNode::Image { alt, .. } => {
            frame.render_widget(Paragraph::new(format!("[image: {alt}]")), area);
        }
        RenderNode::Link { href, child } => {
            let s = format!("{} ({href})", render_inline(child));
            frame.render_widget(Paragraph::new(s), area);
        }
        RenderNode::Callout { kind, child } => {
            let s = format!("[{kind:?}] {}", render_inline(child));
            frame.render_widget(Paragraph::new(s), area);
        }
        RenderNode::Collapsed { summary, .. } => {
            let s = format!("▸ {}", render_inline(summary));
            frame.render_widget(Paragraph::new(s), area);
        }
        RenderNode::Raw { body, .. } => {
            frame.render_widget(Paragraph::new(body.clone()), area);
        }
        RenderNode::Unknown { fallback, .. } => {
            frame.render_widget(Paragraph::new(fallback.text.clone()), area);
        }
    }
}

fn paint_children<B: Backend>(
    frame: &mut ratatui::Frame,
    area: ratatui::layout::Rect,
    children: &[RenderNode],
) {
    if children.is_empty() {
        return;
    }
    let constraints: Vec<Constraint> =
        std::iter::repeat_n(Constraint::Length(1), children.len()).collect();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);
    for (child, chunk) in children.iter().zip(chunks.iter()) {
        paint_node::<B>(frame, *chunk, child);
    }
}

fn render_inline(node: &RenderNode) -> String {
    match node {
        RenderNode::Text { text, .. } => text.clone(),
        RenderNode::Heading { text, .. } => text.clone(),
        RenderNode::Code { code, .. } => code.clone(),
        RenderNode::Inline { children } | RenderNode::Block { children } => children
            .iter()
            .map(render_inline)
            .collect::<Vec<_>>()
            .join(""),
        RenderNode::List { items, .. } => items
            .iter()
            .map(render_inline)
            .collect::<Vec<_>>()
            .join(", "),
        RenderNode::KeyValue { pairs } => pairs
            .iter()
            .map(|p| format!("{}={}", p.key, render_inline(&p.value)))
            .collect::<Vec<_>>()
            .join(", "),
        RenderNode::Table { .. } => "[table]".to_string(),
        RenderNode::Divider {} => "---".to_string(),
        RenderNode::Image { alt, .. } => format!("[image: {alt}]"),
        RenderNode::Link { href, child } => format!("{} ({href})", render_inline(child)),
        RenderNode::Callout { kind, child } => {
            format!("[{kind:?}] {}", render_inline(child))
        }
        RenderNode::Collapsed { summary, .. } => render_inline(summary),
        RenderNode::Raw { body, .. } => body.clone(),
        RenderNode::Unknown { fallback, .. } => fallback.text.clone(),
    }
}

#[cfg(test)]
pub(crate) enum PaintAction<'a> {
    Render(&'a RenderNode),
    RunPanicking,
    RunReturningError,
}

#[cfg(test)]
pub(crate) fn draw_with_panic_isolation_for_test<B: Backend>(
    term: &mut Terminal<B>,
    action: PaintAction<'_>,
) -> Result<(), PaintError> {
    match action {
        PaintAction::Render(node) => draw_with_panic_isolation(term, node),
        PaintAction::RunReturningError => Err(PaintError::Draw(std::io::Error::other(
            "synthetic paint error",
        ))),
        PaintAction::RunPanicking => {
            let result =
                std::panic::catch_unwind(AssertUnwindSafe(|| panic!("synthetic paint panic")));
            match result {
                Ok(()) => Ok(()),
                Err(panic) => {
                    let msg = panic_message(&panic);
                    draw_error_line(term, &msg)
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;

    fn buffer_contains(term: &Terminal<TestBackend>, needle: &str) -> bool {
        let buf = term.backend().buffer();
        let mut joined = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                joined.push_str(buf[(x, y)].symbol());
            }
            joined.push('\n');
        }
        joined.contains(needle)
    }

    #[test]
    fn panicking_paint_renders_error_line_and_next_render_proceeds() {
        let backend = TestBackend::new(60, 4);
        let mut term = Terminal::new(backend).unwrap();

        draw_with_panic_isolation_for_test(&mut term, PaintAction::RunPanicking)
            .expect("panic should be caught and error line drawn");
        assert!(
            buffer_contains(&term, "[render error:"),
            "expected '[render error:' on the test backend after panic"
        );
        assert!(
            buffer_contains(&term, "synthetic paint panic"),
            "expected panic message rendered on the test backend"
        );

        let node = RenderNode::Text {
            text: "hello after panic".to_string(),
            emphasis: None,
        };
        draw_with_panic_isolation_for_test(&mut term, PaintAction::Render(&node))
            .expect("subsequent render should succeed");
        assert!(
            buffer_contains(&term, "hello after panic"),
            "expected next render to proceed normally"
        );
    }

    #[test]
    fn paint_action_returning_error_propagates() {
        let backend = TestBackend::new(40, 2);
        let mut term = Terminal::new(backend).unwrap();
        let err = draw_with_panic_isolation_for_test(&mut term, PaintAction::RunReturningError)
            .expect_err("synthetic error should propagate");
        match err {
            PaintError::Draw(io) => assert_eq!(io.to_string(), "synthetic paint error"),
        }
    }

    #[test]
    fn block_paints_children_text() {
        let backend = TestBackend::new(40, 4);
        let mut term = Terminal::new(backend).unwrap();
        let node = RenderNode::Block {
            children: vec![
                RenderNode::Text {
                    text: "first-line".to_string(),
                    emphasis: None,
                },
                RenderNode::Text {
                    text: "second-line".to_string(),
                    emphasis: None,
                },
            ],
        };
        draw_with_panic_isolation(&mut term, &node).unwrap();
        assert!(buffer_contains(&term, "first-line"));
        assert!(buffer_contains(&term, "second-line"));
    }
}
