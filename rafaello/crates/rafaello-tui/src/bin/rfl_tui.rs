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
use rafaello_tui::paint::draw_with_panic_isolation;
use rafaello_tui::{slash::SlashCommand, slash::SlashKind, CONFIRM_ANSWER_TOPIC};
use ratatui::backend::CrosstermBackend;
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
        if let Some(answer) = cfg.test_confirm_answer {
            spawn_auto_confirm_answer(
                client.peer().clone(),
                confirm_rx,
                answer,
                cfg.test_confirm_delay_ms,
            );
        }
        std::future::pending::<Result<()>>().await
    } else {
        run_production_mode(notifications).await
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

fn publish_submitted_line(client: &Client<OneShotConnector>, text: &str) -> Result<()> {
    let request_id = JsonRpcId::String(Ulid::new().to_string());
    let (topic, payload) = if text.starts_with('/') {
        let cmd = rafaello_tui::slash::parse(text);
        (
            "frontend.tui.slash_command",
            serde_json::to_value(&cmd).expect("SlashCommand serialises"),
        )
    } else {
        ("frontend.tui.user_message", json!({ "text": text }))
    };
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
    mut notifications: broadcast::Receiver<InboundNotification>,
) -> Result<()> {
    install_panic_hook();

    let mut stdout = io::stdout();
    enable_raw_mode().context("enable raw mode")?;
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture).context("enter alternate screen")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("build ratatui terminal")?;

    let result = ui_loop(&mut terminal, &mut notifications).await;

    restore_terminal();
    result
}

async fn ui_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    notifications: &mut broadcast::Receiver<InboundNotification>,
) -> Result<()> {
    let mut buffer: VecDeque<RenderNode> = VecDeque::with_capacity(RENDER_BUFFER_CAP);
    let mut scroll: u16 = 0;
    let mut events = EventStream::new();

    loop {
        let dirty = tokio::select! {
            biased;
            key = events.next() => {
                let Some(ev) = key else { return Ok(()); };
                let ev = ev.context("crossterm event stream error")?;
                match handle_terminal_event(ev, &mut scroll, buffer.len()) {
                    EventOutcome::Quit => return Ok(()),
                    EventOutcome::Redraw => true,
                    EventOutcome::Ignore => false,
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
            redraw(terminal, &buffer, scroll);
        }
    }
}

enum EventOutcome {
    Quit,
    Redraw,
    Ignore,
}

fn handle_terminal_event(ev: Event, scroll: &mut u16, len: usize) -> EventOutcome {
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

fn redraw(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    buffer: &VecDeque<RenderNode>,
    scroll: u16,
) {
    let start = scroll as usize;
    let nodes: Vec<RenderNode> = buffer.iter().skip(start).cloned().collect();
    let frame = RenderNode::Block { children: nodes };
    let _ = draw_with_panic_isolation(terminal, &frame);
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
