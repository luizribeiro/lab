//! c26 / scope §TUI2-§TUI4: multi-pending confirm queue, overlay
//! rendering, and the 1 s TTL countdown driven by
//! `tokio::time::interval`. The countdown is purely UI; deadline
//! enforcement is server-side (§CG5).

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use fittings_core::message::JsonRpcId;
use rafaello_core::PaintError;
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::text::Line;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::{overlay_from_confirm_request, ConfirmDetails, InputMode};

pub const CONFIRM_RESOLVED_TOPIC: &str = "core.session.confirm_resolved";

#[derive(Debug, Clone, PartialEq)]
pub struct PendingConfirm {
    pub confirm_id: JsonRpcId,
    pub summary: String,
    pub details: ConfirmDetails,
    pub ttl_remaining: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConfirmQueue {
    pending: VecDeque<PendingConfirm>,
}

impl ConfirmQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue(&mut self, payload: &Value) -> bool {
        let Some(InputMode::ConfirmOverlay {
            confirm_id,
            summary,
            details,
            ttl_remaining,
            ..
        }) = overlay_from_confirm_request(payload, 0)
        else {
            return false;
        };
        self.pending.push_back(PendingConfirm {
            confirm_id,
            summary,
            details,
            ttl_remaining,
        });
        true
    }

    pub fn head(&self) -> Option<&PendingConfirm> {
        self.pending.front()
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    pub fn queued_count(&self) -> u32 {
        self.pending.len().saturating_sub(1) as u32
    }

    pub fn head_overlay(&self) -> InputMode {
        match self.pending.front() {
            None => InputMode::Normal,
            Some(p) => InputMode::ConfirmOverlay {
                confirm_id: p.confirm_id.clone(),
                summary: p.summary.clone(),
                details: p.details.clone(),
                ttl_remaining: p.ttl_remaining,
                queued_count: self.queued_count(),
            },
        }
    }

    pub fn pop_head(&mut self) -> Option<PendingConfirm> {
        self.pending.pop_front()
    }

    pub fn drop_by_request_id(&mut self, request_id: &str) -> bool {
        let before = self.pending.len();
        self.pending
            .retain(|p| p.confirm_id.as_str() != Some(request_id));
        before != self.pending.len()
    }

    pub fn handle_confirm_resolved(&mut self, payload: &Value) -> bool {
        let Some(id) = payload.get("request_id").and_then(|v| v.as_str()) else {
            return false;
        };
        self.drop_by_request_id(id)
    }

    pub fn handle_confirm_reply(&mut self, payload: &Value) -> bool {
        let Some(id) = payload.get("request_id").and_then(|v| v.as_str()) else {
            return false;
        };
        self.drop_by_request_id(id)
    }

    pub fn tick(&mut self) {
        if let Some(head) = self.pending.front_mut() {
            head.ttl_remaining = head.ttl_remaining.saturating_sub(1);
        }
    }
}

/// Spawnable TTL-countdown driver: a `tokio::time::interval(1s)` ticker
/// that decrements the head entry's `ttl_remaining` once per second.
/// The countdown is purely UI — server-side timeout (§CG5) is
/// authoritative.
pub async fn run_ttl_ticker(queue: Arc<Mutex<ConfirmQueue>>) {
    let mut interval = tokio::time::interval(Duration::from_secs(1));
    interval.tick().await;
    loop {
        interval.tick().await;
        queue.lock().await.tick();
    }
}

/// Paint the overlay frame above the input line. The frame's title
/// includes the `+N more pending` badge when other entries are queued
/// behind the head.
pub fn paint_confirm_overlay<B: Backend>(
    term: &mut Terminal<B>,
    queue: &ConfirmQueue,
) -> Result<(), PaintError> {
    let Some(head) = queue.head() else {
        return Ok(());
    };
    let queued = queue.queued_count();
    let title = if queued > 0 {
        format!(" confirm  +{queued} more pending ")
    } else {
        " confirm ".to_string()
    };
    let sinks = if head.details.sinks.is_empty() {
        "(none)".to_string()
    } else {
        head.details.sinks.join(", ")
    };
    let taint = match &head.details.taint {
        Value::Array(a) if a.is_empty() => "(none)".to_string(),
        v => v.to_string(),
    };
    let args = serde_json::to_string(&head.details.args).unwrap_or_default();
    let lines: Vec<Line> = vec![
        Line::from(head.summary.clone()),
        Line::from(format!("args: {args}")),
        Line::from(format!("sinks: {sinks}")),
        Line::from(format!("taint: {taint}")),
        Line::from(format!("{}s remaining", head.ttl_remaining)),
    ];
    let block_title = title.clone();
    term.draw(|frame| {
        let area = frame.area();
        let h = ((lines.len() as u16) + 2).min(area.height);
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(h)])
            .split(area);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(block_title.clone());
        frame.render_widget(Paragraph::new(lines.clone()).block(block), chunks[1]);
    })
    .map(|_| ())
    .map_err(PaintError::Draw)
}
