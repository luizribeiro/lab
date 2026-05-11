//! c19 / scope §SL4: TUI state for `core.session.command_result`.
//! Slash results are a transient callout above the input, never
//! conversation history.

use std::collections::{HashSet, VecDeque};

use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Terminal;
use serde_json::Value;

use rafaello_core::PaintError;

pub const TOPIC_COMMAND_RESULT: &str = "core.session.command_result";
const MAX_CALLOUTS: usize = 64;

#[derive(Default, Debug)]
pub struct CommandResultState {
    pending: HashSet<String>,
    callouts: VecDeque<String>,
}

impl CommandResultState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn note_pending(&mut self, request_id: impl Into<String>) {
        self.pending.insert(request_id.into());
    }

    pub fn ingest_event(&mut self, params: &Value) -> bool {
        if params.get("topic").and_then(Value::as_str) != Some(TOPIC_COMMAND_RESULT) {
            return false;
        }
        let Some(corr) = params
            .get("in_reply_to")
            .and_then(Value::as_array)
            .and_then(|a| a.first())
            .and_then(Value::as_str)
        else {
            return false;
        };
        if !self.pending.remove(corr) {
            return false;
        }
        let msg = params
            .get("payload")
            .and_then(|p| p.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if self.callouts.len() == MAX_CALLOUTS {
            self.callouts.pop_front();
        }
        self.callouts.push_back(msg);
        true
    }

    pub fn callouts(&self) -> &VecDeque<String> {
        &self.callouts
    }
}

pub fn paint_frame<B: Backend>(
    term: &mut Terminal<B>,
    entries: &[String],
    callouts: &VecDeque<String>,
) -> Result<(), PaintError> {
    term.draw(|frame| {
        let area = frame.area();
        let h = (callouts.len() as u16).min(area.height.saturating_sub(1));
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(h),
                Constraint::Length(1),
            ])
            .split(area);
        let lines: Vec<Line> = entries.iter().map(|s| Line::from(s.clone())).collect();
        frame.render_widget(Paragraph::new(lines), chunks[0]);
        let italic = Style::default().add_modifier(Modifier::ITALIC);
        let call_lines: Vec<Line> = callouts
            .iter()
            .map(|m| Line::from(Span::styled(format!("» {m}"), italic)))
            .collect();
        frame.render_widget(Paragraph::new(call_lines), chunks[1]);
    })
    .map(|_| ())
    .map_err(PaintError::Draw)
}
