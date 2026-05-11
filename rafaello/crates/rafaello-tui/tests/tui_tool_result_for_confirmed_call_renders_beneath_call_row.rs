//! c26 / scope §TUI4: when a `core.session.tool_result` for the
//! confirmed call arrives, the existing m3 entry-update path renders
//! the result row beneath the call row. No overlay-side state
//! mutation is required — the renderer pipeline just paints the new
//! `tool_result` entry as the m3 / m4 pipeline already does.

use rafaello_core::RenderNode;
use rafaello_tui::paint::draw_with_panic_isolation;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn rows(term: &Terminal<TestBackend>) -> Vec<String> {
    let buf = term.backend().buffer();
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width)
                .map(|x| buf[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect()
}

#[test]
fn tool_result_renders_below_tool_call() {
    let call = RenderNode::Text {
        text: "tool_call fs.write(/etc/hosts)".to_string(),
        emphasis: None,
    };
    let result = RenderNode::Text {
        text: "tool_result fs.write ok".to_string(),
        emphasis: None,
    };
    let frame = RenderNode::Block {
        children: vec![call, result],
    };

    let mut term = Terminal::new(TestBackend::new(60, 6)).unwrap();
    draw_with_panic_isolation(&mut term, &frame).unwrap();
    let painted = rows(&term);

    let call_row = painted
        .iter()
        .position(|r| r.contains("tool_call fs.write"))
        .expect("tool_call rendered");
    let result_row = painted
        .iter()
        .position(|r| r.contains("tool_result fs.write"))
        .expect("tool_result rendered");
    assert!(
        result_row > call_row,
        "tool_result must paint beneath tool_call (call row {call_row}, result row {result_row})"
    );
}
