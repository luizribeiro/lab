//! `rfl-tui` — TUI front-end binary (scope §T1, §T2 steps 1–7, §T6).

use std::collections::VecDeque;
use std::io::{self, Write};
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use fittings_client::{Client, InboundNotification};
use fittings_core::message::JsonRpcId;
use fittings_core::{error::FittingsError, transport::Connector};
use fittings_transport::stdio::StdioTransport;
use futures::stream::StreamExt;
use rafaello_core::RenderNode;
use rafaello_tui::env::{self, TestConfirmAnswer, TestGrantBeforeMessage};
use rafaello_tui::paint::draw_with_input_bar;
use rafaello_tui::test_confirm_queue::TestConfirmAnswerQueue;
use rafaello_tui::{slash::SlashCommand, slash::SlashKind, InputMode, CONFIRM_ANSWER_TOPIC};
use ratatui::backend::{Backend, CrosstermBackend};
use ratatui::Terminal;
use serde_json::{json, Value};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use ulid::Ulid;

const MAX_FRAME_BYTES: usize = 1 << 20;
const DEFAULT_MAX_LIFETIME_SECS: u64 = 60;
const RENDER_BUFFER_CAP: usize = 1024;

type BusTransport = StdioTransport<OwnedReadHalf, OwnedWriteHalf>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cfg = env::load().context("rfl-tui: env parsing failed")?;

    emit_sentinel(&format!("project-root={}", cfg.project_root.display()));

    let transport = adopt_bus_fd(cfg.bus_fd).context("rfl-tui: adopt bus fd")?;

    if cfg.test_mode {
        let lifetime = cfg.max_lifetime_secs.unwrap_or(DEFAULT_MAX_LIFETIME_SECS);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(lifetime)).await;
            std::process::exit(0);
        });
    }

    let (confirm_tx, confirm_rx) = tokio::sync::mpsc::unbounded_channel::<serde_json::Value>();
    let handler = bus_event_handler(cfg.test_mode, confirm_tx);

    let client = Client::connect(OneShotConnector::new(transport))
        .await
        .context("rfl-tui: connect fittings client")?
        .with_notification_handler(handler);

    let notifications = client.subscribe_notifications();

    if let Some(ms) = cfg.ready_delay_ms {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    client
        .peer()
        .call("frontend.ready", json!({}))
        .await
        .context("rfl-tui: frontend.ready RPC")?;

    if let Some(grant) = cfg.test_grant_before_message.as_ref() {
        publish_synthetic_grant(&client, grant)?;
    }

    if let Some(text) = cfg.test_message.as_deref() {
        publish_submitted_line(&client, text)?;
    }

    if cfg.test_mode {
        if let Some(answers) = cfg.test_confirm_answers {
            run_plural_test_mode(
                answers,
                client.peer().clone(),
                confirm_rx,
                cfg.test_confirm_delay_ms,
            )
            .await
        } else {
            if let Some(answer) = cfg.test_confirm_answer {
                spawn_auto_confirm_answer(
                    client.peer().clone(),
                    confirm_rx,
                    answer,
                    cfg.test_confirm_delay_ms,
                );
            }
            std::future::pending::<Result<()>>().await
        }
    } else {
        run_production_mode(&client, notifications).await
    }
}

async fn run_plural_test_mode(
    answers: Vec<TestConfirmAnswer>,
    peer: fittings_core::context::PeerHandle,
    confirm_rx: tokio::sync::mpsc::UnboundedReceiver<serde_json::Value>,
    delay_ms: u64,
) -> Result<()> {
    let (fatal_tx, mut fatal_rx) = tokio::sync::oneshot::channel::<String>();
    let queue = Arc::new(TestConfirmAnswerQueue::new(answers, fatal_tx));
    let join_handle = tokio::spawn(run_plural_auto_confirm_loop(
        queue, peer, confirm_rx, delay_ms,
    ));
    tokio::select! {
        res = join_handle => {
            match res {
                Ok(()) => {
                    std::future::pending::<()>().await;
                    unreachable!("plural-auto-confirm loop exited Ok unexpectedly")
                }
                Err(join_err) => {
                    let msg = fatal_rx
                        .try_recv()
                        .ok()
                        .unwrap_or_else(|| {
                            format!("rfl-tui: confirm-answer task panicked: {join_err}")
                        });
                    panic!("{msg}");
                }
            }
        }
        msg = &mut fatal_rx => {
            let msg = msg.unwrap_or_else(|_| "fatal channel closed".to_string());
            panic!("{msg}");
        }
    }
}

async fn run_plural_auto_confirm_loop(
    queue: Arc<TestConfirmAnswerQueue>,
    peer: fittings_core::context::PeerHandle,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<serde_json::Value>,
    delay_ms: u64,
) {
    while let Some(payload) = rx.recv().await {
        let answer = queue.next_answer();
        let Some(answer_str) = answer.answer_str() else {
            continue;
        };
        let Some(confirm_id) = payload.get("request_id").and_then(|v| v.as_str()) else {
            continue;
        };
        let confirm_id = confirm_id.to_string();
        if delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
        let request_id = JsonRpcId::String(Ulid::new().to_string());
        let _ = peer.notify(
            "bus.publish",
            json!({
                "topic": CONFIRM_ANSWER_TOPIC,
                "payload": { "request_id": confirm_id, "answer": answer_str },
                "request_id": request_id,
                "in_reply_to": [JsonRpcId::String(confirm_id.clone())],
            }),
        );
    }
}

fn publish_synthetic_grant(
    client: &Client<OneShotConnector>,
    grant: &TestGrantBeforeMessage,
) -> Result<()> {
    let cmd = SlashCommand {
        command: SlashKind::Grant,
        args: json!({ "tool": grant.tool, "template": grant.args_subset }),
    };
    let payload = serde_json::to_value(&cmd).expect("SlashCommand serialises");
    let request_id = JsonRpcId::String(Ulid::new().to_string());
    client
        .peer()
        .notify(
            "bus.publish",
            json!({
                "topic": "frontend.tui.slash_command",
                "payload": payload,
                "request_id": request_id,
            }),
        )
        .context("rfl-tui: bus.publish frontend.tui.slash_command (synthetic /grant)")?;
    Ok(())
}

fn spawn_auto_confirm_answer(
    peer: fittings_core::context::PeerHandle,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<serde_json::Value>,
    answer: TestConfirmAnswer,
    delay_ms: u64,
) {
    tokio::spawn(async move {
        while let Some(payload) = rx.recv().await {
            let Some(answer_str) = answer.answer_str() else {
                continue;
            };
            let Some(confirm_id) = payload.get("request_id").and_then(|v| v.as_str()) else {
                continue;
            };
            let confirm_id = confirm_id.to_string();
            if delay_ms > 0 {
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }
            let request_id = JsonRpcId::String(Ulid::new().to_string());
            let _ = peer.notify(
                "bus.publish",
                json!({
                    "topic": CONFIRM_ANSWER_TOPIC,
                    "payload": { "request_id": confirm_id, "answer": answer_str },
                    "request_id": request_id,
                    "in_reply_to": [JsonRpcId::String(confirm_id.clone())],
                }),
            );
        }
    });
}

fn submitted_line_envelope(text: &str) -> (&'static str, Value) {
    if text.starts_with('/') {
        let cmd = rafaello_tui::slash::parse(text);
        (
            "frontend.tui.slash_command",
            serde_json::to_value(&cmd).expect("SlashCommand serialises"),
        )
    } else {
        ("frontend.tui.user_message", json!({ "text": text }))
    }
}

fn publish_submitted_line(client: &Client<OneShotConnector>, text: &str) -> Result<()> {
    let request_id = JsonRpcId::String(Ulid::new().to_string());
    let (topic, payload) = submitted_line_envelope(text);
    client
        .peer()
        .notify(
            "bus.publish",
            json!({
                "topic": topic,
                "payload": payload,
                "request_id": request_id,
            }),
        )
        .with_context(|| format!("rfl-tui: bus.publish {topic}"))?;
    Ok(())
}

fn emit_sentinel(line: &str) {
    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(stderr, "{line}");
    let _ = stderr.flush();
}

fn adopt_bus_fd(fd: RawFd) -> Result<BusTransport> {
    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    let std_stream = std::os::unix::net::UnixStream::from(owned);
    std_stream
        .set_nonblocking(true)
        .context("set inherited bus fd to non-blocking")?;
    let stream = tokio::net::UnixStream::from_std(std_stream)
        .context("convert std UnixStream to tokio UnixStream")?;
    let (reader, writer) = stream.into_split();
    Ok(StdioTransport::new(reader, writer, MAX_FRAME_BYTES))
}

fn bus_event_handler(
    test_mode: bool,
    confirm_tx: tokio::sync::mpsc::UnboundedSender<Value>,
) -> impl Fn(String, Value) + Send + Sync + 'static {
    let seq = Arc::new(AtomicU64::new(0));
    move |method, params| {
        if method != "bus.event" {
            return;
        }
        if !test_mode {
            return;
        }
        let topic = params
            .get("topic")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let n = seq.fetch_add(1, Ordering::SeqCst) + 1;
        emit_sentinel(&format!("bus.event topic={topic} seq={n}"));
        if topic == rafaello_tui::CONFIRM_REQUEST_TOPIC {
            if let Some(payload) = params.get("payload") {
                let _ = confirm_tx.send(payload.clone());
            }
        }
        if topic == "core.lifecycle.test_done" {
            emit_sentinel("test-done");
            std::process::exit(0);
        }
    }
}

async fn run_production_mode(
    client: &Client<OneShotConnector>,
    mut notifications: broadcast::Receiver<InboundNotification>,
) -> Result<()> {
    install_panic_hook();

    let mut stdout = io::stdout();
    enable_raw_mode().context("enable raw mode")?;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).context("enter alternate screen")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("build ratatui terminal")?;

    let result = ui_loop(&mut terminal, &mut notifications, client).await;

    restore_terminal();
    result
}

async fn ui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    notifications: &mut broadcast::Receiver<InboundNotification>,
    client: &Client<OneShotConnector>,
) -> Result<()> {
    let mut buffer: VecDeque<RenderNode> = VecDeque::with_capacity(RENDER_BUFFER_CAP);
    let mut scroll: u16 = 0;
    let mut input_buffer: String = String::new();
    let mode: InputMode = InputMode::default();
    let mut events = EventStream::new();

    loop {
        let dirty = tokio::select! {
            biased;
            key = events.next() => {
                let Some(ev) = key else { return Ok(()); };
                let ev = ev.context("crossterm event stream error")?;
                match handle_terminal_event(
                    ev,
                    &mode,
                    &mut scroll,
                    &mut input_buffer,
                    buffer.len(),
                ) {
                    EventOutcome::Quit => return Ok(()),
                    EventOutcome::Redraw => true,
                    EventOutcome::Ignore => false,
                    EventOutcome::Submit(line) => {
                        publish_submitted_line(client, &line)?;
                        true
                    }
                }
            }
            note = notifications.recv() => {
                match note {
                    Ok(n) => ingest_notification(n, &mut buffer),
                    Err(broadcast::error::RecvError::Closed) => return Ok(()),
                    Err(broadcast::error::RecvError::Lagged(_)) => true,
                }
            }
        };

        if dirty {
            redraw(terminal, &buffer, scroll, &input_buffer, &mode);
        }
    }
}

enum EventOutcome {
    Quit,
    Redraw,
    Ignore,
    Submit(String),
}

fn handle_terminal_event(
    ev: Event,
    mode: &InputMode,
    scroll: &mut u16,
    input_buffer: &mut String,
    len: usize,
) -> EventOutcome {
    let Event::Key(key) = ev else {
        return EventOutcome::Ignore;
    };
    if key.kind == KeyEventKind::Release {
        return EventOutcome::Ignore;
    }
    match key.code {
        KeyCode::Char('q') => EventOutcome::Quit,
        KeyCode::Up => {
            *scroll = scroll.saturating_sub(1);
            EventOutcome::Redraw
        }
        KeyCode::Down => {
            let max = len.saturating_sub(1) as u16;
            if *scroll < max {
                *scroll += 1;
            }
            EventOutcome::Redraw
        }
        code => {
            if matches!(mode, InputMode::Normal) {
                handle_normal_key(code, input_buffer)
            } else {
                EventOutcome::Ignore
            }
        }
    }
}

fn handle_normal_key(code: KeyCode, input_buffer: &mut String) -> EventOutcome {
    match code {
        KeyCode::Char(c) => {
            input_buffer.push(c);
            EventOutcome::Redraw
        }
        KeyCode::Backspace => {
            input_buffer.pop();
            EventOutcome::Redraw
        }
        KeyCode::Enter => {
            let line = std::mem::take(input_buffer);
            EventOutcome::Submit(line)
        }
        _ => EventOutcome::Ignore,
    }
}

fn ingest_notification(note: InboundNotification, buffer: &mut VecDeque<RenderNode>) -> bool {
    if note.method != "bus.event" {
        return false;
    }
    let Some(params) = note.params else {
        return false;
    };
    let topic = params.get("topic").and_then(|v| v.as_str()).unwrap_or("");
    if !topic.starts_with("core.entry.") {
        return false;
    }
    let Some(payload) = params.get("payload") else {
        return false;
    };
    let Some(tree) = payload.get("tree") else {
        return false;
    };
    let node: RenderNode = match serde_json::from_value(tree.clone()) {
        Ok(n) => n,
        Err(_) => return false,
    };
    if buffer.len() == RENDER_BUFFER_CAP {
        buffer.pop_front();
    }
    buffer.push_back(node);
    true
}

fn redraw<B: Backend>(
    terminal: &mut Terminal<B>,
    buffer: &VecDeque<RenderNode>,
    scroll: u16,
    input_buffer: &str,
    mode: &InputMode,
) {
    let start = scroll as usize;
    let nodes: Vec<RenderNode> = buffer.iter().skip(start).cloned().collect();
    let frame = RenderNode::Block { children: nodes };
    let show_input_bar = !mode.input_blocked();
    let _ = draw_with_input_bar(terminal, &frame, input_buffer, show_input_bar);
}

fn install_panic_hook() {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default(info);
    }));
}

fn restore_terminal() {
    let mut stdout = io::stdout();
    let _ = execute!(stdout, LeaveAlternateScreen, DisableMouseCapture);
    let _ = disable_raw_mode();
}

struct OneShotConnector {
    transport: Arc<Mutex<Option<BusTransport>>>,
}

impl OneShotConnector {
    fn new(transport: BusTransport) -> Self {
        Self {
            transport: Arc::new(Mutex::new(Some(transport))),
        }
    }
}

#[async_trait]
impl Connector for OneShotConnector {
    type Connection = BusTransport;

    async fn connect(&self) -> Result<Self::Connection, FittingsError> {
        self.transport
            .lock()
            .await
            .take()
            .ok_or_else(|| FittingsError::transport("OneShotConnector::connect called twice"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};
    use rafaello_tui::ConfirmDetails;
    use ratatui::backend::TestBackend;
    use serde_json::json;

    fn key_event(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn terminal_text(term: &Terminal<TestBackend>) -> String {
        let buf = term.backend().buffer();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn last_row(term: &Terminal<TestBackend>) -> String {
        let buf = term.backend().buffer();
        let y = buf.area.height - 1;
        let mut row = String::new();
        for x in 0..buf.area.width {
            row.push_str(buf[(x, y)].symbol());
        }
        row
    }

    #[test]
    fn production_ui_loop_renders_input_bar_with_prompt() {
        let backend = TestBackend::new(40, 5);
        let mut term = Terminal::new(backend).unwrap();
        let buffer: VecDeque<RenderNode> = VecDeque::new();
        let mode = InputMode::Normal;

        redraw(&mut term, &buffer, 0, "hello-world", &mode);

        let row = last_row(&term);
        assert!(
            row.trim_end().starts_with("> hello-world"),
            "expected prompt + input on final row, got: {row:?}"
        );
    }

    #[test]
    fn production_ui_loop_hides_input_bar_when_blocked() {
        let backend = TestBackend::new(40, 5);
        let mut term = Terminal::new(backend).unwrap();
        let buffer: VecDeque<RenderNode> = VecDeque::new();
        let mode = InputMode::ConfirmOverlay {
            confirm_id: JsonRpcId::String("test-confirm".to_string()),
            summary: String::new(),
            details: ConfirmDetails {
                tool_call_id: String::new(),
                tool: String::new(),
                args: json!({}),
                sinks: Vec::new(),
                always_confirm: false,
                taint: json!([]),
            },
            ttl_remaining: 0,
            queued_count: 0,
        };
        assert!(mode.input_blocked());

        redraw(&mut term, &buffer, 0, "should-not-appear", &mode);

        let all = terminal_text(&term);
        assert!(
            !all.contains("should-not-appear"),
            "input contents must be hidden when blocked, got: {all:?}"
        );
        assert!(
            !all.contains("> "),
            "prompt glyph must be hidden when blocked, got: {all:?}"
        );
    }

    #[test]
    fn normal_mode_char_appends_to_input_buffer() {
        let mut input = String::from("hi");
        let mut scroll: u16 = 0;
        let mode = InputMode::Normal;
        let out = handle_terminal_event(
            key_event(KeyCode::Char('!')),
            &mode,
            &mut scroll,
            &mut input,
            0,
        );
        assert!(matches!(out, EventOutcome::Redraw));
        assert_eq!(input, "hi!");
    }

    #[test]
    fn normal_mode_backspace_pops_last_char() {
        let mut input = String::from("foo");
        let mut scroll: u16 = 0;
        let mode = InputMode::Normal;
        let out = handle_terminal_event(
            key_event(KeyCode::Backspace),
            &mode,
            &mut scroll,
            &mut input,
            0,
        );
        assert!(matches!(out, EventOutcome::Redraw));
        assert_eq!(input, "fo");
    }

    #[test]
    fn normal_mode_enter_invokes_publish_submitted_line() {
        let mut input = String::from("hello");
        let mut scroll: u16 = 0;
        let mode = InputMode::Normal;
        let out =
            handle_terminal_event(key_event(KeyCode::Enter), &mode, &mut scroll, &mut input, 0);
        let line = match out {
            EventOutcome::Submit(s) => s,
            _ => panic!("expected Submit outcome for Enter"),
        };
        assert_eq!(line, "hello");
        assert!(input.is_empty(), "Enter must clear input_buffer");
        let (topic, payload) = submitted_line_envelope(&line);
        assert_eq!(topic, "frontend.tui.user_message");
        assert_eq!(payload, json!({ "text": "hello" }));
    }
}
