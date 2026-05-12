//! `rafaello-fetch` — bundled `web-fetch` tool plugin (scope §TF2, §A6).
//!
//! Subscribes to `plugin.<RFL_TOPIC_ID>.tool_request` and publishes
//! `plugin.<RFL_TOPIC_ID>.tool_result` with the canonical wire shape
//! `{ok, content?, error?}`. No real HTTP — body sourced from
//! `RFL_FETCH_TEST_BODY_PATH` per `rafaello_fetch::handle_web_fetch`.

use std::os::fd::{FromRawFd, OwnedFd, RawFd};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use fittings_client::{Client, InboundNotification};
use fittings_core::context::PeerHandle;
use fittings_core::message::JsonRpcId;
use fittings_core::{error::FittingsError, transport::Connector};
use fittings_transport::stdio::StdioTransport;
use rafaello_fetch::compute_publish_params;
use serde_json::Value;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{broadcast, Mutex};

const MAX_FRAME_BYTES: usize = 1 << 20;

type BusTransport = StdioTransport<OwnedReadHalf, OwnedWriteHalf>;

struct Config {
    topic_id: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let fd = parse_bus_fd()?;
    let topic_id = std::env::var("RFL_TOPIC_ID").context("RFL_TOPIC_ID not set")?;

    let cfg = Config { topic_id };

    let transport = adopt_bus_fd(fd).context("rafaello-fetch: adopt bus fd")?;
    let client = Client::connect(OneShotConnector::new(transport))
        .await
        .context("rafaello-fetch: client connect")?;
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
        let Some(reply_to) = bus_request_id else {
            continue;
        };
        let payload = params.get("payload").cloned().unwrap_or(Value::Null);
        let publish_params = compute_publish_params(&payload, reply_to, &result_topic);
        let _ = peer.notify("bus.publish", publish_params);
    }
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
