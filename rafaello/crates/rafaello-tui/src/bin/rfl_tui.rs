//! `rfl-tui` — TUI front-end binary (scope §T1, §T2 steps 1–4).

use std::io::Write;
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use fittings_client::Client;
use fittings_core::{error::FittingsError, transport::Connector};
use fittings_transport::stdio::StdioTransport;
use rafaello_tui::env;
use serde_json::{json, Value};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex;

const MAX_FRAME_BYTES: usize = 1 << 20;
const DEFAULT_MAX_LIFETIME_SECS: u64 = 60;

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

    let handler = bus_event_handler(cfg.test_mode);

    let client = Client::connect(OneShotConnector::new(transport))
        .await
        .context("rfl-tui: connect fittings client")?
        .with_notification_handler(handler);

    if let Some(ms) = cfg.ready_delay_ms {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    client
        .peer()
        .call("frontend.ready", json!({}))
        .await
        .context("rfl-tui: frontend.ready RPC")?;

    std::future::pending::<Result<()>>().await
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

fn bus_event_handler(test_mode: bool) -> impl Fn(String, Value) + Send + Sync + 'static {
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
        if topic == "core.lifecycle.test_done" {
            emit_sentinel("test-done");
            std::process::exit(0);
        }
    }
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
