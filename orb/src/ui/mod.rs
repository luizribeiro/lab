use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

pub mod markdown;
pub mod scrollback;

pub use scrollback::{
    CommitColor, commit_blank_line, commit_dim_line, commit_header, commit_markdown,
    commit_status_line, commit_tool_result, commit_user_prompt,
};

use crate::app::{App, COMPOSER_HEIGHT};

const BRAILLE_TICKS: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn draw(frame: &mut Frame, app: &mut App) {
    // Viewport layout: stacked modals on top (height = sum of their
    // requested heights), composer block pinned to the bottom
    // `COMPOSER_HEIGHT` rows. When no modals are active the viewport is
    // exactly `COMPOSER_HEIGHT` and the modal slot is empty.
    let area = frame.area();
    let composer_height = COMPOSER_HEIGHT.min(area.height);
    let modal_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height.saturating_sub(composer_height),
    };
    let composer_area = Rect {
        x: area.x,
        y: modal_area.bottom(),
        width: area.width,
        height: composer_height,
    };

    if modal_area.height > 0 {
        app.modals.render(modal_area, frame);
    }
    draw_composer_block(frame, composer_area, app);
}

fn draw_composer_block(frame: &mut Frame, area: Rect, app: &App) {
    // codex-style framing: a dim horizontal rule above the composer and one
    // below, no left/right borders. When a turn is active, the top border
    // doubles as a status line: spinner + "Working…" + "esc to interrupt"
    // rendered as a `Block` title, so the status doesn't cost a row.
    let border_style = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::DIM);

    let mut block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(border_style);

    if app.active.is_some() {
        let spinner = current_tick();
        let title = Line::from(vec![
            Span::raw(" "),
            Span::styled(spinner, Style::default().fg(Color::Cyan)),
            Span::raw(" "),
            Span::styled("Working…", Style::default().add_modifier(Modifier::DIM)),
            Span::raw("  "),
            Span::styled(
                "esc to interrupt",
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM),
            ),
            Span::raw(" "),
        ]);
        block = block.title(title);
    }

    if let Some(title) = queued_title(app) {
        block = block.title_bottom(title);
    }

    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(search) = &app.composer.search {
        draw_search_overlay(frame, inner, app, search);
    } else {
        draw_composer(frame, inner, app);
    }
}

fn draw_composer(frame: &mut Frame, area: Rect, app: &App) {
    // tui-textarea handles cursor + multi-line editing for us.
    frame.render_widget(&app.composer.textarea, area);
}

fn draw_search_overlay(frame: &mut Frame, area: Rect, app: &App, search: &crate::composer::Search) {
    let matched = search
        .match_idx
        .and_then(|i| app.composer.history.entries.get(i))
        .map(String::as_str)
        .unwrap_or("");
    let line = Line::from(vec![
        Span::styled("(reverse-i-search)", Style::default().fg(Color::Cyan)),
        Span::raw(" `"),
        Span::styled(
            search.query.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw("': "),
        Span::raw(matched.to_string()),
    ]);
    let hint = Line::from(Span::styled(
        "  enter accept · ctrl+r older · esc cancel",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM),
    ));
    frame.render_widget(Paragraph::new(vec![line, hint]), area);
}

fn queued_title(app: &App) -> Option<Line<'static>> {
    if app.queue.is_empty() {
        return None;
    }

    let mut previews = app
        .queue
        .iter()
        .take(2)
        .map(|prompt| preview_prompt(prompt))
        .collect::<Vec<_>>();
    if app.queue.len() > previews.len() {
        previews.push(format!("+{}", app.queue.len() - previews.len()));
    }

    Some(Line::from(vec![
        Span::raw(" "),
        Span::styled(
            format!("queued {}", app.queue.len()),
            Style::default().fg(Color::Yellow),
        ),
        Span::raw(": "),
        Span::styled(
            previews.join(" · "),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ),
        Span::raw(" "),
    ]))
}

fn preview_prompt(prompt: &str) -> String {
    let compact = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    const MAX_CHARS: usize = 48;
    if compact.chars().count() <= MAX_CHARS {
        return compact;
    }

    let mut truncated = compact.chars().take(MAX_CHARS - 3).collect::<String>();
    truncated.push_str("...");
    truncated
}

fn current_tick() -> &'static str {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    BRAILLE_TICKS[((ms / 80) as usize) % BRAILLE_TICKS.len()]
}
