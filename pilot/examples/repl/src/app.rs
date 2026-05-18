use std::collections::VecDeque;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crossterm::event::{
    Event as CtEvent, EventStream, KeyCode, KeyEventKind, KeyModifiers,
};
use futures_util::StreamExt;
use pilot::{
    Claude, Codex, CodexConfig, Gemini, Pi, Session, TurnItem, TurnOptions,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use uuid::Uuid;

use crate::composer::Composer;
use crate::markdown::MarkdownSkin;
use crate::transcript::Transcript;
use crate::turn::{self, ActiveTurn};
use crate::ui;

pub const VIEWPORT_HEIGHT: u16 = 8;

pub type Term = Terminal<CrosstermBackend<io::Stdout>>;

#[derive(Clone, Copy)]
pub enum AgentKind {
    Claude,
    Codex,
    Gemini,
    Pi,
}

impl AgentKind {
    pub fn label(self) -> &'static str {
        match self {
            AgentKind::Claude => "claude",
            AgentKind::Codex => "codex",
            AgentKind::Gemini => "gemini",
            AgentKind::Pi => "pi",
        }
    }
}

pub struct App {
    pub agent: AgentKind,
    pub session: Session,
    pub transcript: Transcript,
    pub composer: Composer,
    pub active: Option<ActiveTurn>,
    pub queue: VecDeque<String>,
    pub skin: MarkdownSkin,
    pub resumed: bool,
    quit: bool,
}

enum Step {
    Key(CtEvent),
    Item(Result<TurnItem, pilot::Error>),
    Tick,
    Nothing,
}

impl App {
    pub fn new(agent: AgentKind, workdir: &Path, resume: Option<Uuid>) -> Self {
        let session = make_session(agent, workdir, resume);
        let transcript = Transcript::for_session(agent, session.id());
        let composer = Composer::new(history_path());
        Self {
            agent,
            session,
            transcript,
            composer,
            active: None,
            queue: VecDeque::new(),
            skin: MarkdownSkin::new(),
            resumed: resume.is_some(),
            quit: false,
        }
    }

    /// Print the welcome header and replay transcript history (if resuming)
    /// to the terminal's native scrollback, BEFORE the inline viewport is
    /// first painted. This way the user can scroll up to see their old
    /// conversation.
    pub fn boot(&mut self, terminal: &mut Term) -> io::Result<()> {
        ui::commit_header(terminal, self.agent, self.resumed)?;
        if self.resumed {
            self.transcript.replay(terminal, &self.skin)?;
        }
        Ok(())
    }

    pub async fn run(&mut self, terminal: &mut Term) -> io::Result<()> {
        let mut events = EventStream::new();
        loop {
            // Drain a queued prompt whenever we're idle. Doing this here
            // (instead of recursively calling dispatch from handlers) keeps
            // every async fn non-recursive — no Box::pin needed.
            if self.active.is_none()
                && let Some(next) = self.queue.pop_front()
            {
                self.start_turn(next, terminal).await?;
            }

            terminal.draw(|f| ui::draw(f, self))?;

            let tick_active = self.active.is_some();
            let step = tokio::select! {
                ev = events.next() => match ev {
                    Some(Ok(ev)) => Step::Key(ev),
                    _ => Step::Nothing,
                },
                item = turn::poll(&mut self.active) => match item {
                    Some(it) => Step::Item(it),
                    None => Step::Nothing,
                },
                _ = maybe_tick(tick_active) => Step::Tick,
            };

            match step {
                Step::Key(ev) => self.handle_key(ev, terminal).await?,
                Step::Item(it) => self.handle_turn_item(it, terminal).await?,
                Step::Tick | Step::Nothing => {}
            }

            if self.quit {
                break;
            }
        }
        Ok(())
    }

    async fn handle_key(&mut self, ev: CtEvent, terminal: &mut Term) -> io::Result<()> {
        let CtEvent::Key(key) = ev else { return Ok(()); };
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        if self.composer.is_searching() {
            self.composer.handle_search_key(key);
            return Ok(());
        }

        match (key.code, key.modifiers) {
            (KeyCode::Char('d'), m) | (KeyCode::Char('c'), m)
                if m.contains(KeyModifiers::CONTROL) =>
            {
                self.quit = true;
            }
            (KeyCode::Char('r'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.composer.start_search();
            }
            (KeyCode::Char('g'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.composer.open_external_editor(terminal).await?;
            }
            (KeyCode::Esc, _) => {
                self.handle_esc(terminal).await?;
            }
            (KeyCode::Enter, m) if m.contains(KeyModifiers::SHIFT) => {
                // Shift+Enter inserts a literal newline into the textarea.
                self.composer.input(key);
            }
            (KeyCode::Enter, _) => {
                self.submit(terminal).await?;
            }
            _ => {
                self.composer.input(key);
            }
        }
        Ok(())
    }

    async fn handle_esc(&mut self, terminal: &mut Term) -> io::Result<()> {
        // Cancel the active turn (if any) AND drop the queue. Earlier we tried
        // a two-step model (1st Esc cancels, 2nd clears queue), but because
        // the main loop auto-drains the queue between keystrokes, the 2nd Esc
        // always landed against a freshly-dispatched queued prompt instead of
        // the queue itself — racy and surprising. "Esc = stop everything" is
        // the predictable contract.
        let cancelled = self.active.take();
        let dropped = self.queue.len();
        self.queue.clear();

        if let Some(active) = cancelled {
            let _partial = active.stream.cancel().await;
            let msg = if dropped == 0 {
                "(cancelled)".to_string()
            } else {
                format!("(cancelled · dropped {dropped} queued)")
            };
            ui::commit_status_line(terminal, &msg, ui::CommitColor::Warn)?;
        } else if dropped > 0 {
            ui::commit_status_line(
                terminal,
                &format!("(dropped {dropped} queued)"),
                ui::CommitColor::Warn,
            )?;
        }
        Ok(())
    }


    async fn submit(&mut self, terminal: &mut Term) -> io::Result<()> {
        let text = self.composer.take_input();
        if text.is_empty() {
            return Ok(());
        }
        self.composer.history.push(text.clone());
        if self.active.is_some() {
            self.queue.push_back(text);
        } else {
            self.start_turn(text, terminal).await?;
        }
        Ok(())
    }

    /// Send `prompt` to the session and set up the in-flight state.
    /// If the driver rejects the send, we commit an error line and leave
    /// `self.active = None` so the main loop picks up the next queued prompt.
    async fn start_turn(&mut self, prompt: String, terminal: &mut Term) -> io::Result<()> {
        ui::commit_user_prompt(terminal, &prompt)?;
        match self.session.send(prompt.clone(), TurnOptions::default()).await {
            Ok(stream) => {
                self.active = Some(ActiveTurn::new(stream, prompt));
            }
            Err(e) => {
                ui::commit_status_line(
                    terminal,
                    &format!("send failed: {e}"),
                    ui::CommitColor::Err,
                )?;
            }
        }
        Ok(())
    }

    async fn handle_turn_item(
        &mut self,
        item: Result<TurnItem, pilot::Error>,
        terminal: &mut Term,
    ) -> io::Result<()> {
        match item {
            Ok(TurnItem::Event(ev)) => {
                if let Some(active) = self.active.as_mut() {
                    turn::process_event(active, ev, terminal)?;
                }
            }
            Ok(TurnItem::Complete(_)) => {
                if let Some(active) = self.active.take() {
                    let trimmed = active.text_buffer.trim();
                    if !trimmed.is_empty() {
                        ui::commit_markdown(terminal, &self.skin, trimmed)?;
                        let _ = self.transcript.append_turn(&active.prompt, trimmed);
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                self.active = None;
                ui::commit_status_line(
                    terminal,
                    &format!("turn error: {e}"),
                    ui::CommitColor::Err,
                )?;
            }
        }
        Ok(())
    }

    pub fn print_resume_hint(&self) {
        println!(
            "\nResume this session with: cargo run -p pilot-repl -- --agent {} --resume {}",
            self.agent.label(),
            self.session.id()
        );
    }
}

async fn maybe_tick(enabled: bool) {
    if enabled {
        tokio::time::sleep(Duration::from_millis(80)).await;
    } else {
        std::future::pending::<()>().await;
    }
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
