//! `rfl-mailcat` — bundled `send-mail` tool plugin (scope §TP1–§TP4).
//!
//! Subscribes to `plugin.<RFL_TOPIC_ID>.tool_request` and publishes
//! `plugin.<RFL_TOPIC_ID>.tool_result` with the canonical wire shape
//! `{ok, error?}`. Appends each request payload to `mailcat.log`
//! under `RFL_PRIVATE_STATE_DIR` (auto-granted per
//! `decisions.md` row 16/37). No actual SMTP.

use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use fittings_client::{Client, InboundNotification};
use fittings_core::context::PeerHandle;
use fittings_core::message::JsonRpcId;
use fittings_core::{error::FittingsError, transport::Connector};
use fittings_transport::stdio::StdioTransport;
use rafaello_mailcat::handle_tool_request;
use serde_json::{json, Value};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{broadcast, Mutex};
use ulid::Ulid;

const MAX_FRAME_BYTES: usize = 1 << 20;

type BusTransport = StdioTransport<OwnedReadHalf, OwnedWriteHalf>;

struct Config {
    topic_id: String,
    private_state_dir: PathBuf,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let fd = parse_bus_fd()?;
    let topic_id = std::env::var("RFL_TOPIC_ID").context("RFL_TOPIC_ID not set")?;
    let private_state_dir = PathBuf::from(
        std::env::var("RFL_PRIVATE_STATE_DIR").context("RFL_PRIVATE_STATE_DIR not set")?,
    );

    let cfg = Config {
        topic_id,
        private_state_dir,
    };

    let transport = adopt_bus_fd(fd).context("rfl-mailcat: adopt bus fd")?;
    let client = Client::connect(OneShotConnector::new(transport))
        .await
        .context("rfl-mailcat: client connect")?;
    let notifications = client.subscribe_notifications();
    let peer = client.peer();

    run_loop(notifications, peer, cfg).await
}

async fn run_loop(
    mut notifications: broadcast::Receiver<InboundNotification>,
    peer: PeerHandle,
    cfg: Config,
) -> Result<()> {
    let expected_topic = format!("plugin.{}.tool_request", cfg.topic_id);
    let result_topic = format!("plugin.{}.tool_result", cfg.topic_id);
    loop {
        let note = match notifications.recv().await {
            Ok(n) => n,
            Err(broadcast::error::RecvError::Closed) => return Ok(()),
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
        };
        if note.method != "bus.event" {
            continue;
        }
        let Some(params) = note.params else { continue };
        let topic = params
            .get("topic")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        if topic != expected_topic {
            continue;
        }
        let bus_request_id: Option<JsonRpcId> = params
            .get("request_id")
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        let payload = params.get("payload").cloned().unwrap_or(Value::Null);
        let response = handle_tool_request(&payload, &cfg.private_state_dir);
        let Some(reply_to) = bus_request_id else {
            continue;
        };
        publish_result(&peer, &result_topic, reply_to, response);
    }
}

fn publish_result(peer: &PeerHandle, topic: &str, reply_to: JsonRpcId, payload: Value) {
    let request_id = JsonRpcId::String(Ulid::new().to_string());
    let _ = peer.notify(
        "bus.publish",
        json!({
            "topic": topic,
            "payload": payload,
            "request_id": request_id,
            "in_reply_to": [reply_to],
        }),
    );
}

fn parse_bus_fd() -> Result<RawFd> {
    let raw = std::env::var("RFL_BUS_FD").context("RFL_BUS_FD not set")?;
    let fd: RawFd = raw
        .parse()
        .with_context(|| format!("RFL_BUS_FD must be a non-negative integer (got {raw:?})"))?;
    if fd < 0 {
        return Err(anyhow!(
            "RFL_BUS_FD must be a non-negative integer (got {fd})"
        ));
    }
    Ok(fd)
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

struct OneShotConnector {
    transport: Mutex<Option<BusTransport>>,
}

impl OneShotConnector {
    fn new(transport: BusTransport) -> Self {
        Self {
            transport: Mutex::new(Some(transport)),
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
