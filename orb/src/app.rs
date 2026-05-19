use std::collections::VecDeque;
use std::io;
use std::path::Path;
use std::time::Duration;

use crossterm::event::{Event as CtEvent, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures_util::StreamExt;
use pilot::{Session, TurnItem, TurnOptions};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use uuid::Uuid;

use crate::agent::{self, AgentKind};
use crate::commands::{self, CommandResult};
use crate::composer::Composer;
use crate::markdown::MarkdownSkin;
use crate::modal::{ModalEffect, ModalResult, ModalStack};
use crate::transcript::Transcript;
use crate::turn::{self, ActiveTurn};
use crate::ui;
use crate::utils;

/// Height of the composer block: top bar + 1 textarea row + bottom bar.
/// When a turn is in flight, the spinner label is embedded *inside* the
/// top border (codex-style title), so the status doesn't need a row of
/// its own.
pub const COMPOSER_HEIGHT: u16 = 3;

/// Default inline-viewport height when no modals are stacked. Equal to
/// COMPOSER_HEIGHT; modals push the viewport taller on demand.
pub const VIEWPORT_HEIGHT: u16 = COMPOSER_HEIGHT;

pub type Term = Terminal<CrosstermBackend<io::Stdout>>;

/// Build a fresh `Terminal` with an inline viewport of `height` rows,
/// anchored at the current cursor position.
pub fn make_terminal(height: u16) -> io::Result<Term> {
    let backend = CrosstermBackend::new(io::stdout());
    Terminal::with_options(
        backend,
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Inline(height),
        },
    )
}

pub struct App {
    pub agent: AgentKind,
    pub model: Option<String>,
    pub session: Session,
    pub transcript: Transcript,
    pub composer: Composer,
    pub active: Option<ActiveTurn>,
    pub queue: VecDeque<String>,
    pub skin: MarkdownSkin,
    pub modals: ModalStack,
    pub resumed: bool,
    /// Current inline-viewport height (rows). Tracked here so the run loop
    /// can spot when modal pushes/pops change the desired height and resync
    /// the terminal — see `sync_viewport_height`.
    viewport_height: u16,
    /// Set by `/redraw`. The next run-loop tick re-runs the viewport resync
    /// dance unconditionally, which rebuilds the terminal and EventStream
    /// from scratch and gets the user out of any wedged-rendering state.
    force_resync: bool,
    quit: bool,
}

enum Step {
    Key(CtEvent),
    Item(Result<TurnItem, pilot::Error>),
    Tick,
    Nothing,
}

impl App {
    pub fn new(
        agent: AgentKind,
        workdir: &Path,
        resume: Option<Uuid>,
        model_override: Option<String>,
    ) -> Self {
        let model = model_override.or_else(|| agent.default_model().map(String::from));
        let session = agent::make_session(agent, workdir, resume, model.clone());
        let transcript = Transcript::for_session(agent, session.id());
        let composer = Composer::new(utils::history_path());
        Self {
            agent,
            model,
            session,
            transcript,
            composer,
            active: None,
            queue: VecDeque::new(),
            skin: MarkdownSkin::new(),
            modals: ModalStack::default(),
            resumed: resume.is_some(),
            viewport_height: VIEWPORT_HEIGHT,
            force_resync: false,
            quit: false,
        }
    }

    /// Print the welcome header and replay transcript history (if resuming)
    /// to the terminal's native scrollback, BEFORE the inline viewport is
    /// first painted. This way the user can scroll up to see their old
    /// conversation.
    pub fn boot(&mut self, terminal: &mut Term) -> io::Result<()> {
        ui::commit_header(
            terminal,
            self.agent,
            self.model.as_deref(),
            self.session.workdir(),
            self.session.id(),
            self.resumed,
        )?;
        if self.resumed {
            self.transcript.replay(terminal, &self.skin)?;
        }
        Ok(())
    }

    pub async fn run(&mut self, terminal: &mut Term) -> io::Result<()> {
        // Events live inside an Option so we can drop the EventStream
        // around viewport resizes — ratatui's inline-viewport resize calls
        // `crossterm::cursor::position()` (ESC[6n), and EventStream would
        // otherwise eat the response from stdin.
        let mut events = Some(EventStream::new());
        loop {
            // Drain a queued prompt whenever we're idle. Doing this here
            // (instead of recursively calling dispatch from handlers) keeps
            // every async fn non-recursive — no Box::pin needed.
            if self.active.is_none()
                && let Some(next) = self.queue.pop_front()
            {
                self.start_turn(next, terminal).await?;
            }

            self.sync_viewport_height(terminal, &mut events)?;
            terminal.draw(|f| ui::draw(f, self))?;

            let tick_active = self.active.is_some();
            let step = {
                let stream = events.as_mut().expect("events always present here");
                tokio::select! {
                    ev = stream.next() => match ev {
                        Some(Ok(ev)) => Step::Key(ev),
                        _ => Step::Nothing,
                    },
                    item = turn::poll(&mut self.active) => match item {
                        Some(it) => Step::Item(it),
                        None => Step::Nothing,
                    },
                    _ = maybe_tick(tick_active) => Step::Tick,
                }
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

    /// Resize the inline viewport whenever the modal stack's desired height
    /// differs from the current viewport height.
    ///
    /// The viewport resize path inside ratatui sends an `ESC[6n` cursor query
    /// to the terminal and reads its reply from stdin. If crossterm's
    /// EventStream is still attached, it races for that reply and the reported
    /// cursor row comes back stale, leaving the new viewport painted at the
    /// wrong row. We work around that by dropping the EventStream before the
    /// resize and recreating it afterwards.
    fn sync_viewport_height(
        &mut self,
        terminal: &mut Term,
        events: &mut Option<EventStream>,
    ) -> io::Result<()> {
        let desired = self.desired_viewport_height();
        if desired == self.viewport_height && !self.force_resync {
            return Ok(());
        }
        self.force_resync = false;
        events.take();
        // Clear the old viewport so cells freed by a shrink don't linger.
        let _ = terminal.clear();
        *terminal = make_terminal(desired)?;
        *events = Some(EventStream::new());
        self.viewport_height = desired;
        Ok(())
    }

    pub fn desired_viewport_height(&self) -> u16 {
        // The frame width isn't known to App, but modal heights are typically
        // computed from row count rather than width. Pass a permissive width;
        // height() implementations clamp themselves later in the renderer.
        let modal_height = self.modals.total_height(u16::MAX);
        COMPOSER_HEIGHT.saturating_add(modal_height)
    }

    fn apply_modal_effect(&mut self, effect: ModalEffect) {
        match effect {
            ModalEffect::ReplaceComposer(text) => {
                self.composer.replace_text(text);
            }
        }
    }

    async fn handle_key(&mut self, ev: CtEvent, terminal: &mut Term) -> io::Result<()> {
        let key = match ev {
            CtEvent::FocusGained => {
                self.composer.set_focused(true);
                return Ok(());
            }
            CtEvent::FocusLost => {
                self.composer.set_focused(false);
                return Ok(());
            }
            CtEvent::Key(k) => k,
            _ => return Ok(()),
        };
        if key.kind != KeyEventKind::Press {
            return Ok(());
        }

        if self.composer.is_searching() {
            self.composer.handle_search_key(key);
            return Ok(());
        }

        // Top-of-stack modal sees keys first. Consumed/Dismiss short-circuit;
        // Forward falls through to the normal composer routing below.
        if !self.modals.is_empty() {
            let (result, effect) = self.modals.handle_key(key);
            if let Some(effect) = effect {
                self.apply_modal_effect(effect);
            }
            match result {
                ModalResult::Consumed | ModalResult::Dismiss => {
                    self.broadcast_composer_change();
                    return Ok(());
                }
                ModalResult::Forward => {}
            }
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
                self.broadcast_composer_change();
            }
            (KeyCode::Enter, _) => {
                self.submit(terminal).await?;
            }
            (KeyCode::Up, KeyModifiers::NONE) => {
                self.composer.history_previous();
                self.broadcast_composer_change();
            }
            (KeyCode::Down, KeyModifiers::NONE) => {
                self.composer.history_next();
                self.broadcast_composer_change();
            }
            _ => {
                self.composer.input(key);
                self.broadcast_composer_change();
            }
        }
        Ok(())
    }

    fn broadcast_composer_change(&mut self) {
        if self.modals.is_empty() {
            return;
        }
        let text = self.composer.textarea.lines().join("\n");
        self.modals.on_composer_change(&text);
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

        // Slash commands intercept the line before it reaches the agent.
        if let Some(cmd) = commands::parse(&text) {
            self.run_command(cmd, terminal).await?;
            return Ok(());
        }
        // Surface a friendly error if the user typed a slash that didn't match.
        if text.starts_with('/') {
            ui::commit_status_line(
                terminal,
                &format!("unknown command: {text}"),
                ui::CommitColor::Err,
            )?;
            return Ok(());
        }

        if self.active.is_some() {
            self.queue.push_back(text);
        } else {
            self.start_turn(text, terminal).await?;
        }
        Ok(())
    }

    async fn run_command(
        &mut self,
        cmd: &commands::Command,
        terminal: &mut Term,
    ) -> io::Result<()> {
        match (cmd.handler)(self) {
            CommandResult::Continue => {}
            CommandResult::Quit => {
                self.quit = true;
            }
            CommandResult::Redraw => {
                self.force_resync = true;
            }
            CommandResult::StartNewSession => {
                self.start_new_session(terminal).await?;
            }
        }
        Ok(())
    }

    async fn start_new_session(&mut self, terminal: &mut Term) -> io::Result<()> {
        if let Some(active) = self.active.take() {
            let _ = active.stream.cancel().await;
        }
        self.queue.clear();

        let old_id = self.session.id();
        let workdir = self.session.workdir().to_path_buf();
        self.session = agent::make_session(self.agent, &workdir, None, self.model.clone());
        self.resumed = false;
        self.transcript = Transcript::for_session(self.agent, self.session.id());

        ui::commit_status_line(
            terminal,
            &format!("(new session {} — replaced {old_id})", self.session.id()),
            ui::CommitColor::Warn,
        )?;
        Ok(())
    }

    /// Send `prompt` to the session and set up the in-flight state.
    /// If the driver rejects the send, we commit an error line and leave
    /// `self.active = None` so the main loop picks up the next queued prompt.
    async fn start_turn(&mut self, prompt: String, terminal: &mut Term) -> io::Result<()> {
        ui::commit_user_prompt(terminal, &prompt)?;
        match self
            .session
            .send(prompt.clone(), TurnOptions::default())
            .await
        {
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
                    turn::process_event(active, ev, terminal, &self.skin)?;
                }
            }
            Ok(TurnItem::Complete(_)) => {
                if let Some(mut active) = self.active.take() {
                    turn::flush_pending_text(&mut active, terminal, &self.skin)?;
                    if !active.transcript_entries.is_empty() {
                        let _ = self
                            .transcript
                            .append_turn(&active.prompt, &active.transcript_entries);
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
            "\nResume this session with: orb --agent {} --resume {}",
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


