//! Interactive ratatui chat that drives any of the four built-in pilot
//! drivers. Run with:
//!
//!     cargo run --example chat -- --agent claude
//!
//! Keys: Enter submits, Esc cancels the in-flight turn, Ctrl-C / Ctrl-D quit.

use std::path::PathBuf;

use crossterm::event::{
    Event as CEvent, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use futures_util::StreamExt;
use pilot::{Claude, Codex, Event, Gemini, Pi, Session, TurnItem, TurnOptions, TurnStream};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use tokio::sync::mpsc;

#[derive(Clone, Copy)]
enum AgentKind {
    Claude,
    Codex,
    Gemini,
    Pi,
}

impl AgentKind {
    fn label(self) -> &'static str {
        match self {
            AgentKind::Claude => "claude",
            AgentKind::Codex => "codex",
            AgentKind::Gemini => "gemini",
            AgentKind::Pi => "pi",
        }
    }
}

enum HistoryLine {
    User(String),
    Assistant(String),
    ToolCall {
        name: String,
        args: serde_json::Value,
    },
    ToolResult {
        ok: bool,
        output: String,
    },
    System(String),
}

enum TurnEvent {
    Event(Event),
    Done,
    Error(String),
}

struct TurnHandle {
    rx: mpsc::Receiver<TurnEvent>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TurnHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct App {
    agent: AgentKind,
    session: Session,
    history: Vec<HistoryLine>,
    input: String,
    streaming: bool,
}

enum Action {
    None,
    Quit,
    Submit(String),
    Cancel,
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let agent = parse_args();
    run_app(agent).await
}

async fn run_app(agent: AgentKind) -> std::io::Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp"));
    let session = match agent {
        AgentKind::Claude => Session::new(Claude::new(), &cwd),
        AgentKind::Codex => Session::new(Codex::new(), &cwd),
        AgentKind::Gemini => Session::new(Gemini::new(), &cwd),
        AgentKind::Pi => Session::new(Pi::new(), &cwd),
    };
    let mut app = App {
        agent,
        session,
        history: Vec::new(),
        input: String::new(),
        streaming: false,
    };
    let mut terminal = ratatui::init();
    let result = main_loop(&mut terminal, &mut app).await;
    ratatui::restore();
    result
}

async fn main_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    let mut keys = EventStream::new();
    let mut turn: Option<TurnHandle> = None;

    loop {
        terminal.draw(|f| ui(f, app))?;
        tokio::select! {
            Some(ev) = keys.next() => {
                let ev = match ev {
                    Ok(e) => e,
                    Err(e) => {
                        app.history.push(HistoryLine::System(format!("input error: {e}")));
                        continue;
                    }
                };
                match handle_key(app, ev) {
                    Action::Quit => break,
                    Action::Submit(prompt) => {
                        if turn.is_some() {
                            app.history.push(HistoryLine::System(
                                "(turn already in flight; press Esc to cancel)".into(),
                            ));
                            continue;
                        }
                        app.history.push(HistoryLine::User(prompt.clone()));
                        match app.session.send(prompt, TurnOptions::default()).await {
                            Ok(stream) => {
                                turn = Some(spawn_turn(stream));
                                app.streaming = true;
                            }
                            Err(e) => {
                                app.history.push(HistoryLine::System(format!("send failed: {e}")));
                            }
                        }
                    }
                    Action::Cancel => {
                        if turn.take().is_some() {
                            app.streaming = false;
                            app.history.push(HistoryLine::System("(cancelled)".into()));
                        }
                    }
                    Action::None => {}
                }
            }
            ev = recv_or_pending(&mut turn) => {
                apply_turn_event(app, ev);
                if !app.streaming {
                    turn = None;
                }
            }
        }
    }

    Ok(())
}

async fn recv_or_pending(turn: &mut Option<TurnHandle>) -> TurnEvent {
    match turn {
        None => std::future::pending().await,
        Some(h) => h.rx.recv().await.unwrap_or(TurnEvent::Done),
    }
}

fn spawn_turn(mut stream: TurnStream) -> TurnHandle {
    let (tx, rx) = mpsc::channel(64);
    let task = tokio::spawn(async move {
        while let Some(item) = stream.next().await {
            let msg = match item {
                Ok(TurnItem::Event(e)) => TurnEvent::Event(e),
                Ok(TurnItem::Complete(_)) => TurnEvent::Done,
                Ok(_) => continue,
                Err(e) => TurnEvent::Error(format!("{e}")),
            };
            let terminal = matches!(msg, TurnEvent::Done | TurnEvent::Error(_));
            if tx.send(msg).await.is_err() {
                return;
            }
            if terminal {
                return;
            }
        }
        let _ = tx.send(TurnEvent::Done).await;
    });
    TurnHandle { rx, task }
}

fn apply_turn_event(app: &mut App, ev: TurnEvent) {
    match ev {
        TurnEvent::Event(Event::AssistantText { delta }) => {
            if let Some(HistoryLine::Assistant(s)) = app.history.last_mut() {
                s.push_str(&delta);
            } else {
                app.history.push(HistoryLine::Assistant(delta));
            }
        }
        TurnEvent::Event(Event::ToolCall { name, args, .. }) => {
            app.history.push(HistoryLine::ToolCall { name, args });
        }
        TurnEvent::Event(Event::ToolResult { ok, output, .. }) => {
            app.history.push(HistoryLine::ToolResult { ok, output });
        }
        TurnEvent::Event(Event::TurnComplete { ok }) => {
            if !ok {
                app.history
                    .push(HistoryLine::System("(turn failed)".into()));
            }
        }
        TurnEvent::Event(_) => {}
        TurnEvent::Done => {
            app.streaming = false;
        }
        TurnEvent::Error(s) => {
            app.history.push(HistoryLine::System(format!("error: {s}")));
            app.streaming = false;
        }
    }
}

fn handle_key(app: &mut App, ev: CEvent) -> Action {
    let CEvent::Key(KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        ..
    }) = ev
    else {
        return Action::None;
    };
    if modifiers.contains(KeyModifiers::CONTROL)
        && matches!(code, KeyCode::Char('c') | KeyCode::Char('d'))
    {
        return Action::Quit;
    }
    match code {
        KeyCode::Esc => Action::Cancel,
        KeyCode::Enter => {
            let prompt = app.input.trim().to_string();
            app.input.clear();
            if prompt.is_empty() {
                Action::None
            } else {
                Action::Submit(prompt)
            }
        }
        KeyCode::Backspace => {
            app.input.pop();
            Action::None
        }
        KeyCode::Char(c) => {
            app.input.push(c);
            Action::None
        }
        _ => Action::None,
    }
}

fn ui(f: &mut Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(3)])
        .split(area);

    let title = format!(" pilot chat — {} ", app.agent.label());
    let footer_hint = " esc cancel · ^C quit ";
    let chat_block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(Span::styled(
            title,
            Style::default().add_modifier(Modifier::BOLD),
        )))
        .title_bottom(
            Line::from(Span::styled(
                footer_hint,
                Style::default().fg(Color::DarkGray),
            ))
            .right_aligned(),
        );

    let inner_width = chunks[0].width.saturating_sub(2) as usize;
    let lines = render_history(&app.history);
    let total = wrapped_count(&lines, inner_width);
    let chat_height = chunks[0].height.saturating_sub(2) as usize;
    let scroll = total.saturating_sub(chat_height) as u16;

    let chat = Paragraph::new(lines)
        .block(chat_block)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    f.render_widget(chat, chunks[0]);

    let prompt_prefix = if app.streaming { "…" } else { ">" };
    let input_line = Line::from(vec![
        Span::styled(
            format!("{prompt_prefix} "),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(app.input.as_str()),
        Span::styled("_", Style::default().add_modifier(Modifier::REVERSED)),
    ]);
    let input = Paragraph::new(input_line).block(Block::default().borders(Borders::ALL));
    f.render_widget(input, chunks[1]);
}

fn render_history(history: &[HistoryLine]) -> Vec<Line<'_>> {
    let mut out: Vec<Line> = Vec::with_capacity(history.len() * 2);
    for line in history {
        match line {
            HistoryLine::User(s) => {
                out.push(Line::from(vec![
                    Span::styled(
                        "you  ",
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("> "),
                    Span::raw(s.as_str()),
                ]));
            }
            HistoryLine::Assistant(s) => {
                out.push(Line::from(vec![
                    Span::styled(
                        "agent  ",
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(s.as_str()),
                ]));
            }
            HistoryLine::ToolCall { name, args } => {
                let summary = summarize_args(args);
                out.push(Line::from(vec![
                    Span::styled(
                        format!("  [tool: {name}] "),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled(
                        summary,
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::DIM),
                    ),
                ]));
            }
            HistoryLine::ToolResult { ok, output } => {
                let (color, label) = if *ok {
                    (Color::DarkGray, "  [result] ")
                } else {
                    (Color::Red, "  [error]  ")
                };
                out.push(Line::from(vec![
                    Span::styled(label, Style::default().fg(color)),
                    Span::styled(truncate(output, 200), Style::default().fg(color)),
                ]));
            }
            HistoryLine::System(s) => {
                out.push(Line::from(Span::styled(
                    s.as_str(),
                    Style::default().fg(Color::Red),
                )));
            }
        }
        out.push(Line::raw(""));
    }
    out
}

fn wrapped_count(lines: &[Line<'_>], width: usize) -> usize {
    if width == 0 {
        return lines.len();
    }
    lines
        .iter()
        .map(|l| {
            let w = l.width().max(1);
            w.div_ceil(width).max(1)
        })
        .sum()
}

fn summarize_args(args: &serde_json::Value) -> String {
    match args {
        serde_json::Value::Object(map) => {
            let mut parts = Vec::new();
            for (k, v) in map.iter().take(3) {
                let val = match v {
                    serde_json::Value::String(s) => truncate(s, 60),
                    other => truncate(&other.to_string(), 60),
                };
                parts.push(format!("{k}={val}"));
            }
            parts.join("  ")
        }
        other => truncate(&other.to_string(), 120),
    }
}

fn truncate(s: &str, max: usize) -> String {
    let collapsed: String = s.chars().map(|c| if c == '\n' { ' ' } else { c }).collect();
    let mut out = String::with_capacity(collapsed.len().min(max + 1));
    for (i, c) in collapsed.chars().enumerate() {
        if i >= max {
            out.push('…');
            return out;
        }
        out.push(c);
    }
    out
}

fn parse_args() -> AgentKind {
    let mut agent = AgentKind::Claude;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--agent" => {
                let v = args.next().unwrap_or_else(|| {
                    eprintln!("--agent requires a value: claude|codex|gemini|pi");
                    std::process::exit(2);
                });
                agent = match v.as_str() {
                    "claude" => AgentKind::Claude,
                    "codex" => AgentKind::Codex,
                    "gemini" => AgentKind::Gemini,
                    "pi" => AgentKind::Pi,
                    other => {
                        eprintln!("unknown agent: {other}");
                        std::process::exit(2);
                    }
                };
            }
            "--help" | "-h" => {
                println!("Usage: chat [--agent claude|codex|gemini|pi]");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown arg: {other}");
                std::process::exit(2);
            }
        }
    }
    agent
}
