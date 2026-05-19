use std::io;
use std::path::Path;

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget, Wrap};
use uuid::Uuid;

use crate::agent::AgentKind;
use crate::app::Term;
use crate::ui::markdown::MarkdownSkin;
use crate::utils::{abbreviate_home, git_branch};

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

pub fn commit_header(
    terminal: &mut Term,
    agent: AgentKind,
    model: Option<&str>,
    cwd: &Path,
    session_id: Uuid,
    resumed: bool,
) -> io::Result<()> {
    let model_label = model.unwrap_or("(default)");
    let title = Line::from(vec![
        Span::styled("orb", Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(" · "),
        Span::styled(agent.label(), Style::default().fg(Color::Cyan)),
        Span::raw(" — "),
        Span::styled(model_label, Style::default().fg(Color::Magenta)),
    ]);

    let cwd_line = meta_line("cwd", format_cwd(cwd));
    let mut session_spans = vec![meta_label("session"), Span::raw(session_id.to_string())];
    if resumed {
        session_spans.push(Span::raw(" "));
        session_spans.push(Span::styled(
            "(resumed)",
            Style::default().fg(Color::Yellow),
        ));
    }
    let session_line = Line::from(session_spans);

    let hint = Line::from(Span::styled(
        "↑/↓ history · ctrl+r search · ctrl+g $EDITOR · enter submit · shift+enter newline · esc cancel · ctrl+d quit",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM),
    ));

    insert_lines(
        terminal,
        vec![
            title,
            cwd_line,
            session_line,
            Line::raw(""),
            hint,
            Line::raw(""),
        ],
    )
}

fn meta_line(label: &str, value: String) -> Line<'static> {
    Line::from(vec![meta_label(label), Span::raw(value)])
}

fn meta_label(label: &str) -> Span<'static> {
    const LABEL_WIDTH: usize = 12;
    Span::styled(
        format!("{label:<LABEL_WIDTH$}"),
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM),
    )
}

fn format_cwd(cwd: &Path) -> String {
    let pretty = abbreviate_home(cwd);
    match git_branch(cwd) {
        Some(branch) => format!("{pretty} on {branch}"),
        None => pretty,
    }
}

pub fn commit_user_prompt(terminal: &mut Term, prompt: &str) -> io::Result<()> {
    let mut lines: Vec<Line> = prompt
        .lines()
        .enumerate()
        .map(|l| {
            let (idx, l) = l;
            let prefix = if idx == 0 {
                Span::styled("» ", Style::default().fg(Color::Cyan))
            } else {
                Span::raw("  ")
            };
            Line::from(vec![prefix, Span::raw(l.to_string())])
        })
        .collect();
    if lines.is_empty() {
        lines.push(Line::from(Span::styled(
            "» ",
            Style::default().fg(Color::Cyan),
        )));
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

pub fn commit_blank_line(terminal: &mut Term) -> io::Result<()> {
    insert_lines(terminal, vec![Line::raw("")])
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
