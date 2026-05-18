//! Interactive REPL that drives any of the four built-in pilot drivers.
//!
//! Features:
//!   - Markdown rendering for assistant replies (termimad)
//!   - Working… spinner with per-tool-call status lines (indicatif)
//!   - Esc / Ctrl+C cancels the in-flight turn; Ctrl+D quits
//!   - rustyline prompt: Ctrl+R history search · Ctrl+G edit in $EDITOR
//!   - History persists at ~/.pilothistory
//!   - --resume <uuid> continues a previous session; the command to resume
//!     the current session is printed on exit
//!
//! Run with:
//!     cargo run -p pilot-repl -- --agent claude
//!     cargo run -p pilot-repl -- --agent claude --resume 6e7…

use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossterm::cursor::{MoveDown, MoveLeft, MoveRight, MoveToColumn, MoveUp};
use crossterm::event::{
    Event as CtEvent, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers,
};
use crossterm::terminal::{Clear, ClearType, size};
use crossterm::QueueableCommand;
use futures_util::StreamExt;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle, TermLike};
use pilot::{
    Claude, Codex, CodexConfig, Event, Gemini, Pi, Session, TurnItem, TurnOptions,
};
use rustyline::error::ReadlineError;
use rustyline::{
    Cmd, ConditionalEventHandler, DefaultEditor, Event as RlEvent, EventContext,
    EventHandler, KeyEvent as RlKey, Movement, RepeatCount,
};
use serde::{Deserialize, Serialize};
use termimad::MadSkin;
use uuid::Uuid;

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

struct Args {
    agent: AgentKind,
    resume: Option<Uuid>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp"));
    let mut session = make_session(args.agent, &cwd, args.resume);
    let transcript = Transcript::for_session(args.agent, session.id());

    let history_path = history_path();
    let mut rl: DefaultEditor = DefaultEditor::new()?;
    rl.bind_sequence(
        RlEvent::from(RlKey::ctrl('g')),
        EventHandler::Conditional(Box::new(ExternalEditor)),
    );
    if history_path.exists() {
        let _ = rl.load_history(&history_path);
    }

    let skin = make_skin();
    print_header(args.agent, args.resume.is_some());
    if args.resume.is_some() {
        replay_transcript(&transcript, &skin);
    }

    loop {
        let line = match rl.readline("\x1b[36m»\x1b[0m ") {
            Ok(l) => l,
            Err(ReadlineError::Interrupted) => continue,
            Err(ReadlineError::Eof) => break,
            Err(e) => {
                eprintln!("readline error: {e}");
                break;
            }
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let _ = rl.add_history_entry(line.as_str());
        let _ = rl.save_history(&history_path);

        if let Err(e) = run_turn(&mut session, trimmed, &skin, &transcript).await {
            eprintln!("\x1b[31merror:\x1b[0m {e}");
        }
    }

    print_resume_hint(args.agent, session.id());
    Ok(())
}

fn make_session(agent: AgentKind, workdir: &Path, resume: Option<Uuid>) -> Session {
    match agent {
        AgentKind::Claude => match resume {
            Some(id) => Session::resume(Claude::new(), id, workdir),
            None => Session::new(Claude::new(), workdir),
        },
        AgentKind::Codex => {
            let mut cfg = CodexConfig::default();
            cfg.state.thread_store_path = Some(codex_thread_store());
            let driver = Codex::with_config(cfg);
            match resume {
                Some(id) => Session::resume(driver, id, workdir),
                None => Session::new(driver, workdir),
            }
        }
        AgentKind::Gemini => match resume {
            Some(id) => Session::resume(Gemini::new(), id, workdir),
            None => Session::new(Gemini::new(), workdir),
        },
        AgentKind::Pi => match resume {
            Some(id) => Session::resume(Pi::new(), id, workdir),
            None => Session::new(Pi::new(), workdir),
        },
    }
}

async fn run_turn(
    session: &mut Session,
    prompt: &str,
    skin: &MadSkin,
    transcript: &Transcript,
) -> Result<(), Box<dyn std::error::Error>> {
    let stream = session.send(prompt.to_string(), TurnOptions::default()).await?;

    let raw_was_on = crossterm::terminal::is_raw_mode_enabled().unwrap_or(false);
    if !raw_was_on {
        crossterm::terminal::enable_raw_mode()?;
    }

    let mp = MultiProgress::with_draw_target(ProgressDrawTarget::term_like(Box::new(RawTerm)));
    let main_bar = mp.add(ProgressBar::new_spinner());
    main_bar.set_style(main_spinner_style());
    main_bar.set_message("Working…");
    main_bar.enable_steady_tick(Duration::from_millis(80));

    let mut text = String::new();
    let mut tool_bars: HashMap<String, ProgressBar> = HashMap::new();
    let outcome = drive_stream(stream, &mut text, &mp, &main_bar, &mut tool_bars).await;

    main_bar.finish_and_clear();
    for (_, bar) in tool_bars.drain() {
        bar.finish_and_clear();
    }
    let _ = mp.clear();

    if !raw_was_on {
        let _ = crossterm::terminal::disable_raw_mode();
    }

    match outcome {
        TurnOutcome::Complete => {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                println!();
                skin.print_text(trimmed);
                let _ = transcript.append_turn(prompt, trimmed);
            }
        }
        TurnOutcome::Cancelled => {
            println!("\x1b[33m(cancelled)\x1b[0m");
        }
        TurnOutcome::Failed(msg) => {
            println!("\x1b[31m(error: {msg})\x1b[0m");
        }
    }
    Ok(())
}

enum TurnOutcome {
    Complete,
    Cancelled,
    Failed(String),
}

async fn drive_stream(
    mut stream: pilot::TurnStream,
    text: &mut String,
    mp: &MultiProgress,
    main_bar: &ProgressBar,
    tool_bars: &mut HashMap<String, ProgressBar>,
) -> TurnOutcome {
    let mut events = EventStream::new();
    loop {
        tokio::select! {
            item = stream.next() => match item {
                Some(Ok(TurnItem::Event(ev))) => handle_event(ev, text, mp, main_bar, tool_bars),
                Some(Ok(TurnItem::Complete(_))) => return TurnOutcome::Complete,
                Some(Ok(_)) => {}
                Some(Err(e)) => return TurnOutcome::Failed(format!("{e}")),
                None => return TurnOutcome::Complete,
            },
            input = events.next() => {
                if is_cancel(input) {
                    let _ = stream.cancel().await;
                    return TurnOutcome::Cancelled;
                }
            }
        }
    }
}

fn is_cancel(ev: Option<io::Result<CtEvent>>) -> bool {
    let Some(Ok(CtEvent::Key(KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        ..
    }))) = ev
    else {
        return false;
    };
    matches!(code, KeyCode::Esc)
        || (modifiers.contains(KeyModifiers::CONTROL)
            && matches!(code, KeyCode::Char('c') | KeyCode::Char('C')))
}

fn handle_event(
    ev: Event,
    text: &mut String,
    mp: &MultiProgress,
    main_bar: &ProgressBar,
    tool_bars: &mut HashMap<String, ProgressBar>,
) {
    match ev {
        Event::AssistantText { delta } => text.push_str(&delta),
        Event::ToolCall { call_id, name, .. } => {
            let pb = mp.insert_before(main_bar, ProgressBar::new_spinner());
            pb.set_style(tool_pending_style());
            pb.set_message(name);
            pb.enable_steady_tick(Duration::from_millis(80));
            tool_bars.insert(call_id, pb);
        }
        Event::ToolResult { call_id, ok, .. } => {
            if let Some(pb) = tool_bars.remove(&call_id) {
                let name = pb.message();
                pb.set_style(if ok { tool_ok_style() } else { tool_err_style() });
                pb.finish_with_message(name);
            }
        }
        Event::TurnComplete { ok: false } => {
            let _ = mp.println("\x1b[31m(turn reported failure)\x1b[0m");
        }
        _ => {}
    }
}

fn main_spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.cyan} \x1b[2m{msg}\x1b[0m")
        .expect("valid template")
        .tick_strings(&BRAILLE_TICKS)
}

fn tool_pending_style() -> ProgressStyle {
    ProgressStyle::with_template("  {spinner:.yellow} \x1b[33m{msg}\x1b[0m")
        .expect("valid template")
        .tick_strings(&BRAILLE_TICKS)
}

fn tool_ok_style() -> ProgressStyle {
    ProgressStyle::with_template("  \x1b[32m✓\x1b[0m \x1b[2m{msg}\x1b[0m")
        .expect("valid template")
}

fn tool_err_style() -> ProgressStyle {
    ProgressStyle::with_template("  \x1b[31m✗\x1b[0m \x1b[31m{msg}\x1b[0m")
        .expect("valid template")
}

const BRAILLE_TICKS: [&str; 10] = [
    "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏",
];

fn make_skin() -> MadSkin {
    MadSkin::default_dark()
}

fn print_header(agent: AgentKind, resumed: bool) {
    let suffix = if resumed { " (resumed)" } else { "" };
    println!(
        "\x1b[1mpilot repl\x1b[0m — \x1b[36m{}\x1b[0m{}  \x1b[2m· Ctrl+R history search · Ctrl+G edit in $EDITOR · Esc cancel turn · Ctrl+D quit\x1b[0m",
        agent.label(),
        suffix
    );
}

fn print_resume_hint(agent: AgentKind, session_id: Uuid) {
    println!(
        "\n\x1b[2mResume this session with:\x1b[0m cargo run -p pilot-repl -- --agent {} --resume {}",
        agent.label(),
        session_id
    );
}

fn history_path() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".pilothistory"))
        .unwrap_or_else(|| PathBuf::from(".pilothistory"))
}

fn codex_thread_store() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let dir = home.join(".pilot");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("codex-threads.json")
}

/// indicatif's default draw target writes bare `\n` between lines, which
/// staircases in raw mode (\n moves down but doesn't return carriage). We
/// substitute write_line so that indicatif's MultiProgress renders cleanly
/// while we have raw mode enabled to read keypresses.
#[derive(Debug)]
struct RawTerm;

impl TermLike for RawTerm {
    fn width(&self) -> u16 {
        size().map(|(w, _)| w).unwrap_or(80)
    }
    fn height(&self) -> u16 {
        size().map(|(_, h)| h).unwrap_or(24)
    }
    fn move_cursor_up(&self, n: usize) -> io::Result<()> {
        if n == 0 {
            return Ok(());
        }
        let mut out = io::stderr().lock();
        out.queue(MoveUp(n as u16))?;
        out.flush()
    }
    fn move_cursor_down(&self, n: usize) -> io::Result<()> {
        if n == 0 {
            return Ok(());
        }
        let mut out = io::stderr().lock();
        out.queue(MoveDown(n as u16))?;
        out.flush()
    }
    fn move_cursor_right(&self, n: usize) -> io::Result<()> {
        if n == 0 {
            return Ok(());
        }
        let mut out = io::stderr().lock();
        out.queue(MoveRight(n as u16))?;
        out.flush()
    }
    fn move_cursor_left(&self, n: usize) -> io::Result<()> {
        if n == 0 {
            return Ok(());
        }
        let mut out = io::stderr().lock();
        out.queue(MoveLeft(n as u16))?;
        out.flush()
    }
    fn write_line(&self, s: &str) -> io::Result<()> {
        let mut out = io::stderr().lock();
        out.write_all(s.as_bytes())?;
        out.write_all(b"\r\n")?;
        out.flush()
    }
    fn write_str(&self, s: &str) -> io::Result<()> {
        let mut out = io::stderr().lock();
        out.write_all(s.as_bytes())?;
        out.flush()
    }
    fn clear_line(&self) -> io::Result<()> {
        let mut out = io::stderr().lock();
        out.queue(MoveToColumn(0))?;
        out.queue(Clear(ClearType::CurrentLine))?;
        out.flush()
    }
    fn flush(&self) -> io::Result<()> {
        io::stderr().lock().flush()
    }
}

struct ExternalEditor;

impl ConditionalEventHandler for ExternalEditor {
    fn handle(
        &self,
        _: &RlEvent,
        _: RepeatCount,
        _: bool,
        ctx: &EventContext<'_>,
    ) -> Option<Cmd> {
        let initial = ctx.line().to_string();
        match edit_in_external_editor(&initial) {
            Ok(new) => Some(Cmd::Replace(Movement::WholeBuffer, Some(new))),
            Err(_) => Some(Cmd::Noop),
        }
    }
}

fn edit_in_external_editor(initial: &str) -> io::Result<String> {
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());

    let mut tmp = tempfile::Builder::new()
        .prefix("pilot-prompt-")
        .suffix(".md")
        .tempfile()?;
    tmp.write_all(initial.as_bytes())?;
    tmp.flush()?;
    let (file, path) = tmp.keep().map_err(io::Error::other)?;
    drop(file);

    let raw_was_on = crossterm::terminal::is_raw_mode_enabled().unwrap_or(false);
    if raw_was_on {
        let _ = crossterm::terminal::disable_raw_mode();
    }
    let status_result = std::process::Command::new(&editor).arg(&path).status();
    if raw_was_on {
        let _ = crossterm::terminal::enable_raw_mode();
    }

    let status = status_result?;
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = std::fs::remove_file(&path);

    if !status.success() {
        return Err(io::Error::other("editor exited non-zero"));
    }
    Ok(content.trim_end_matches('\n').to_string())
}

struct Transcript {
    path: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
enum TranscriptEntry {
    User { content: String },
    Assistant { content: String },
}

impl Transcript {
    fn for_session(agent: AgentKind, id: Uuid) -> Self {
        let dir = transcripts_dir();
        let _ = std::fs::create_dir_all(&dir);
        Self {
            path: dir.join(format!("{}-{}.jsonl", agent.label(), id)),
        }
    }

    fn append_turn(&self, user: &str, assistant: &str) -> io::Result<()> {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(
            f,
            "{}",
            serde_json::to_string(&TranscriptEntry::User {
                content: user.to_string()
            })
            .map_err(io::Error::other)?
        )?;
        writeln!(
            f,
            "{}",
            serde_json::to_string(&TranscriptEntry::Assistant {
                content: assistant.to_string()
            })
            .map_err(io::Error::other)?
        )?;
        Ok(())
    }

    fn load(&self) -> Vec<TranscriptEntry> {
        let Ok(content) = std::fs::read_to_string(&self.path) else {
            return Vec::new();
        };
        content
            .lines()
            .filter_map(|l| serde_json::from_str::<TranscriptEntry>(l).ok())
            .collect()
    }
}

fn transcripts_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".pilot").join("transcripts")
}

fn replay_transcript(transcript: &Transcript, skin: &MadSkin) {
    let entries = transcript.load();
    if entries.is_empty() {
        return;
    }
    let turns = entries
        .iter()
        .filter(|e| matches!(e, TranscriptEntry::Assistant { .. }))
        .count();
    let label = if turns == 1 { "turn" } else { "turns" };
    println!("\x1b[2m── conversation so far ({turns} {label}) ──\x1b[0m");
    for entry in entries {
        match entry {
            TranscriptEntry::User { content } => {
                for line in content.lines() {
                    println!("\x1b[36m»\x1b[0m {line}");
                }
            }
            TranscriptEntry::Assistant { content } => {
                println!();
                skin.print_text(&content);
            }
        }
    }
    println!("\x1b[2m── end of history ──\x1b[0m\n");
}

fn parse_args() -> Args {
    let mut agent = AgentKind::Claude;
    let mut resume = None;
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
            "--resume" => {
                let v = args.next().unwrap_or_else(|| {
                    eprintln!("--resume requires a session UUID");
                    std::process::exit(2);
                });
                resume = Some(Uuid::parse_str(&v).unwrap_or_else(|e| {
                    eprintln!("invalid --resume UUID: {e}");
                    std::process::exit(2);
                }));
            }
            "--help" | "-h" => {
                println!(
                    "Usage: pilot-repl [--agent claude|codex|gemini|pi] [--resume <uuid>]"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown arg: {other}");
                std::process::exit(2);
            }
        }
    }
    Args { agent, resume }
}
