use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::Frame;
use ratatui::layout::Rect;
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
    // The viewport is always 3 rows: top bar + 1 textarea row + bottom
    // bar. When a turn is in flight, the spinner + "Working… esc to
    // interrupt" label is rendered as a `Block` title *inside* the top
    // border (codex-style), so we don't need a separate status row.
    draw_composer_block(frame, frame.area(), app);
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
    // ratskin tends to append a blank line of its own; collapse any
    // trailing blanks so our explicit between-turn separator below is
    // always exactly one row.
    while lines
        .last()
        .is_some_and(|l| l.spans.iter().all(|s| s.content.trim().is_empty()))
    {
        lines.pop();
    }
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
