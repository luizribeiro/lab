//! `rfl-tui` — TUI front-end binary (scope §T1, §T2 steps 1–3).

use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use fittings_client::{Client, InboundNotification};
use fittings_core::{error::FittingsError, transport::Connector};
use fittings_transport::stdio::StdioTransport;
use rafaello_tui::env;
use serde_json::{json, Value};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{mpsc, Mutex};

const MAX_FRAME_BYTES: usize = 1 << 20;

type BusTransport = StdioTransport<OwnedReadHalf, OwnedWriteHalf>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cfg = env::load().context("rfl-tui: env parsing failed")?;
    let transport = adopt_bus_fd(cfg.bus_fd).context("rfl-tui: adopt bus fd")?;

    let (event_tx, _event_rx) = mpsc::unbounded_channel::<InboundNotification>();
    let handler = bus_event_handler(event_tx);

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

    // c24 is build-only: production rendering and headless test-mode exit
    // semantics land in c25+. Park the runtime so the process stays attached
    // to the bus until a future commit installs the real lifecycle.
    let _ = cfg.project_root;
    let _ = cfg.test_mode;
    let _ = cfg.max_lifetime_secs;
    std::future::pending::<Result<()>>().await
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
    tx: mpsc::UnboundedSender<InboundNotification>,
) -> impl Fn(String, Value) + Send + Sync + 'static {
    move |method, params| {
        if method != "bus.event" {
            return;
        }
        let _ = tx.send(InboundNotification {
            method,
            params: Some(params),
        });
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
