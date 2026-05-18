use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};

use crate::app::{AgentKind, App, Term};
use crate::markdown::MarkdownSkin;

const BRAILLE_TICKS: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub enum CommitColor {
    Warn,
    Err,
}

impl CommitColor {
    fn into_color(self) -> Color {
        match self {
            CommitColor::Warn => Color::Yellow,
            CommitColor::Err => Color::Red,
        }
    }
}

pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let status_h = status_height(app);
    // Composer block height = textarea lines (capped) + 2 border rows,
    // pinned to the *bottom* of the viewport. Status sits above. The
    // gap in between is padding (occurs when the textarea is small but
    // the viewport is the fixed `VIEWPORT_HEIGHT`).
    let textarea_lines = app.composer.textarea.lines().len().max(1) as u16;
    let composer_h = (textarea_lines + 2).min(area.height.saturating_sub(status_h));
    let pad_h = area
        .height
        .saturating_sub(status_h)
        .saturating_sub(composer_h);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(status_h),
            Constraint::Length(pad_h),
            Constraint::Length(composer_h),
        ])
        .split(area);

    if status_h > 0 {
        draw_status(frame, chunks[0], app);
    }
    draw_composer_block(frame, chunks[2], app);
}

fn draw_composer_block(frame: &mut Frame, area: Rect, app: &App) {
    // codex-style framing: a dim horizontal rule above the composer and one
    // below, no left/right borders. Block::inner gives us the interior rect
    // for the textarea / search overlay to draw into.
    let block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        );
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if let Some(search) = &app.composer.search {
        draw_search_overlay(frame, inner, app, search);
    } else {
        draw_composer(frame, inner, app);
    }
}

pub fn status_height(app: &App) -> u16 {
    if app.active.is_some() { 1 } else { 0 }
}

fn draw_status(frame: &mut Frame, area: Rect, _app: &App) {
    let spinner = current_tick();
    let line = Line::from(vec![
        Span::styled(format!("{spinner} "), Style::default().fg(Color::Cyan)),
        Span::styled("Working…", Style::default().add_modifier(Modifier::DIM)),
        Span::raw("  "),
        Span::styled(
            "esc to interrupt",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_composer(frame: &mut Frame, area: Rect, app: &App) {
    // tui-textarea handles cursor + multi-line editing for us.
    frame.render_widget(&app.composer.textarea, area);
}

fn draw_search_overlay(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    search: &crate::composer::Search,
) {
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

fn current_tick() -> &'static str {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    BRAILLE_TICKS[((ms / 80) as usize) % BRAILLE_TICKS.len()]
}

// ---------- scrollback commit helpers (use terminal.insert_before) ----------

pub fn commit_header(terminal: &mut Term, agent: AgentKind, resumed: bool) -> io::Result<()> {
    let suffix = if resumed { " (resumed)" } else { "" };
    let bar = Line::from(vec![
        Span::styled(
            "pilot repl",
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(" — "),
        Span::styled(agent.label(), Style::default().fg(Color::Cyan)),
        Span::styled(suffix, Style::default().fg(Color::Yellow)),
    ]);
    let hint = Line::from(Span::styled(
        "ctrl+r history · ctrl+g $EDITOR · enter submit · shift+enter newline · esc cancel · ctrl+d quit",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM),
    ));
    insert_lines(terminal, vec![bar, hint, Line::raw("")])
}

pub fn commit_user_prompt(terminal: &mut Term, prompt: &str) -> io::Result<()> {
    let mut lines: Vec<Line> = prompt
        .lines()
        .map(|l| {
            Line::from(vec![
                Span::styled("» ", Style::default().fg(Color::Cyan)),
                Span::raw(l.to_string()),
            ])
        })
        .collect();
    if lines.is_empty() {
        lines.push(Line::from(Span::styled("» ", Style::default().fg(Color::Cyan))));
    }
    lines.push(Line::raw(""));
    insert_lines(terminal, lines)
}

pub fn commit_markdown(terminal: &mut Term, skin: &MarkdownSkin, text: &str) -> io::Result<()> {
    let (width, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let mut lines = skin.render(text, width);
    lines.push(Line::raw(""));
    insert_lines(terminal, lines)
}

pub fn commit_tool_result(terminal: &mut Term, name: &str, ok: bool) -> io::Result<()> {
    let (icon, color) = if ok {
        ("✓", Color::Green)
    } else {
        ("✗", Color::Red)
    };
    let line = Line::from(vec![
        Span::raw("  "),
        Span::styled(icon, Style::default().fg(color)),
        Span::raw(" "),
        Span::styled(
            name.to_string(),
            Style::default().add_modifier(Modifier::DIM),
        ),
    ]);
    insert_lines(terminal, vec![line])
}

pub fn commit_status_line(terminal: &mut Term, msg: &str, color: CommitColor) -> io::Result<()> {
    let line = Line::from(Span::styled(
        msg.to_string(),
        Style::default().fg(color.into_color()),
    ));
    insert_lines(terminal, vec![line])
}

pub fn commit_dim_line(terminal: &mut Term, msg: &str) -> io::Result<()> {
    let line = Line::from(Span::styled(
        msg.to_string(),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM),
    ));
    insert_lines(terminal, vec![line])
}

fn insert_lines(terminal: &mut Term, lines: Vec<Line<'_>>) -> io::Result<()> {
    let height = lines.len() as u16;
    if height == 0 {
        return Ok(());
    }
    terminal.insert_before(height, |buf| {
        let area = buf.area;
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .render(area, buf);
    })?;
    Ok(())
}
