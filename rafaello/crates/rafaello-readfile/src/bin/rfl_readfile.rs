//! `rfl-readfile` — bundled read-file tool plugin (scope §TP2).
//!
//! Subscribes to `plugin.<RFL_TOPIC_ID>.tool_request` and publishes
//! `plugin.<RFL_TOPIC_ID>.tool_result` with the canonical wire shape
//! `{ok, content | error}`. Resolves relative paths against
//! `RFL_PROJECT_ROOT` and rejects paths that canonicalize outside
//! that root (pi-1 H-3 plugin-level negative). `RFL_READFILE_TEST_BYPASS_GUARD=1`
//! skips the in-plugin ancestor check so the sandbox-level denial
//! can be observed in isolation.

use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use fittings_client::{Client, InboundNotification};
use fittings_core::context::PeerHandle;
use fittings_core::message::JsonRpcId;
use fittings_core::{error::FittingsError, transport::Connector};
use fittings_transport::stdio::StdioTransport;
use serde_json::{json, Value};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{broadcast, Mutex};
use ulid::Ulid;

const MAX_FRAME_BYTES: usize = 1 << 20;

type BusTransport = StdioTransport<OwnedReadHalf, OwnedWriteHalf>;

struct Config {
    topic_id: String,
    project_root: PathBuf,
    bypass_guard: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let fd = parse_bus_fd()?;
    let topic_id = std::env::var("RFL_TOPIC_ID").context("RFL_TOPIC_ID not set")?;
    let project_root =
        PathBuf::from(std::env::var("RFL_PROJECT_ROOT").context("RFL_PROJECT_ROOT not set")?);
    let bypass_guard = std::env::var("RFL_READFILE_TEST_BYPASS_GUARD")
        .ok()
        .as_deref()
        == Some("1");

    let cfg = Config {
        topic_id,
        project_root,
        bypass_guard,
    };

    let transport = adopt_bus_fd(fd).context("rfl-readfile: adopt bus fd")?;
    let client = Client::connect(OneShotConnector::new(transport))
        .await
        .context("rfl-readfile: client connect")?;
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
        let outcome = handle_tool_request(&cfg, &payload);
        let Some(reply_to) = bus_request_id else {
            continue;
        };
        publish_result(&peer, &result_topic, reply_to, outcome);
    }
}

enum Outcome {
    Ok(String),
    Err(String),
}

fn handle_tool_request(cfg: &Config, payload: &Value) -> Outcome {
    let tool = payload.get("tool").and_then(|v| v.as_str());
    if tool != Some("read-file") {
        return Outcome::Err(format!("unsupported tool: {}", tool.unwrap_or("<missing>")));
    }
    let Some(raw_path) = payload
        .get("args")
        .and_then(|a| a.get("path"))
        .and_then(|v| v.as_str())
    else {
        return Outcome::Err("missing args.path".to_string());
    };

    let target = if cfg.bypass_guard {
        PathBuf::from(raw_path)
    } else {
        match resolve_within_root(&cfg.project_root, raw_path) {
            Ok(p) => p,
            Err(reason) => return Outcome::Err(reason),
        }
    };

    match std::fs::read(&target) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(s) => Outcome::Ok(s),
            Err(_) => Outcome::Err("file is not valid utf-8".to_string()),
        },
        Err(e) => Outcome::Err(format!("io error: {}: {}", e.kind(), e)),
    }
}

fn resolve_within_root(project_root: &Path, raw_path: &str) -> Result<PathBuf, String> {
    let joined = if Path::new(raw_path).is_absolute() {
        PathBuf::from(raw_path)
    } else {
        project_root.join(raw_path)
    };
    let canon_target = std::fs::canonicalize(&joined).map_err(|e| format!("io error: {e}"))?;
    let canon_root = std::fs::canonicalize(project_root)
        .map_err(|e| format!("io error resolving project root: {e}"))?;
    if !canon_target.starts_with(&canon_root) {
        return Err("path denied".to_string());
    }
    Ok(canon_target)
}

fn publish_result(peer: &PeerHandle, topic: &str, reply_to: JsonRpcId, outcome: Outcome) {
    let payload = match outcome {
        Outcome::Ok(content) => json!({"ok": true, "content": content}),
        Outcome::Err(reason) => json!({"ok": false, "error": reason}),
    };
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
