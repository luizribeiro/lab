//! `rfl-mockprovider` — deterministic mock provider plugin (scope §PR2 + §PR4).
//!
//! Reads `bus.event` notifications, runs a hand-written
//! content-pattern matcher (no regex; multibyte-UTF-8 safe per pi-1
//! M-3 / pi-2 M2-3), holds per-session state, and publishes
//! `provider.mock.tool_request` / `provider.mock.assistant_message`.

use std::collections::{HashMap, HashSet};
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::sync::Arc;

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
const PREFIXES: &[&str] = &["what's in ", "what is in "];
const PATH_PUNCT: &[char] = &['.', '?', '!', ',', ';', ':'];

type BusTransport = StdioTransport<OwnedReadHalf, OwnedWriteHalf>;

struct State {
    outstanding: HashMap<JsonRpcId, String>,
    last_user_message: Option<JsonRpcId>,
    seen_tool_results: HashSet<JsonRpcId>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let fd = parse_bus_fd()?;
    let provider_id = std::env::var("RFL_PROVIDER_ID").unwrap_or_else(|_| "mock".to_string());

    let transport = adopt_bus_fd(fd).context("rfl-mockprovider: adopt bus fd")?;
    let client = Client::connect(OneShotConnector::new(transport))
        .await
        .context("rfl-mockprovider: client connect")?;
    let notifications = client.subscribe_notifications();

    let state = Arc::new(Mutex::new(State {
        outstanding: HashMap::new(),
        last_user_message: None,
        seen_tool_results: HashSet::new(),
    }));
    let peer = client.peer();

    run_loop(notifications, state, peer, provider_id).await
}

async fn run_loop(
    mut notifications: broadcast::Receiver<InboundNotification>,
    state: Arc<Mutex<State>>,
    peer: PeerHandle,
    provider_id: String,
) -> Result<()> {
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
        handle_bus_event(&state, &peer, &provider_id, params).await;
    }
}

async fn handle_bus_event(
    state: &Arc<Mutex<State>>,
    peer: &PeerHandle,
    provider_id: &str,
    params: Value,
) {
    let topic = params
        .get("topic")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let payload = params.get("payload").cloned().unwrap_or(Value::Null);
    let bus_request_id: Option<JsonRpcId> = params
        .get("request_id")
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let in_reply_to: Option<Vec<JsonRpcId>> = params
        .get("in_reply_to")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    match topic.as_str() {
        "core.session.user_message" => {
            handle_user_message(state, peer, provider_id, payload, bus_request_id).await;
        }
        "core.session.tool_result" => {
            handle_tool_result(
                state,
                peer,
                provider_id,
                payload,
                bus_request_id,
                in_reply_to,
            )
            .await;
        }
        _ => {}
    }
}

async fn handle_user_message(
    state: &Arc<Mutex<State>>,
    peer: &PeerHandle,
    provider_id: &str,
    payload: Value,
    bus_request_id: Option<JsonRpcId>,
) {
    let text = payload
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let mut st = state.lock().await;
    if let Some(rid) = bus_request_id.clone() {
        st.last_user_message = Some(rid);
    }
    let fresh = fresh_request_id();

    if let Some(path) = match_read_file(&text) {
        st.outstanding.insert(fresh.clone(), path.clone());
        let in_reply_to: Vec<JsonRpcId> = st.seen_tool_results.iter().cloned().collect();
        drop(st);
        let topic = format!("provider.{provider_id}.tool_request");
        let _ = peer.notify(
            "bus.publish",
            json!({
                "topic": topic,
                "payload": {"tool": "read-file", "args": {"path": path}},
                "request_id": fresh,
                "in_reply_to": in_reply_to,
            }),
        );
    } else {
        let in_reply_to: Vec<JsonRpcId> = st.last_user_message.iter().cloned().collect();
        drop(st);
        let topic = format!("provider.{provider_id}.assistant_message");
        let _ = peer.notify(
            "bus.publish",
            json!({
                "topic": topic,
                "payload": {"text": format!("echo: {text}")},
                "request_id": fresh,
                "in_reply_to": in_reply_to,
            }),
        );
    }
}

async fn handle_tool_result(
    state: &Arc<Mutex<State>>,
    peer: &PeerHandle,
    provider_id: &str,
    payload: Value,
    bus_request_id: Option<JsonRpcId>,
    in_reply_to: Option<Vec<JsonRpcId>>,
) {
    let content = payload
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let cited = in_reply_to.as_ref().and_then(|v| v.first()).cloned();

    let mut st = state.lock().await;
    if let Some(rid) = bus_request_id.clone() {
        st.seen_tool_results.insert(rid);
    }
    let Some(cited_id) = cited else {
        return;
    };
    let Some(path) = st.outstanding.get(&cited_id).cloned() else {
        return;
    };
    drop(st);

    let Some(reply_to) = bus_request_id else {
        return;
    };
    let fresh = fresh_request_id();
    let topic = format!("provider.{provider_id}.assistant_message");
    let _ = peer.notify(
        "bus.publish",
        json!({
            "topic": topic,
            "payload": {"text": format!("Here's what's in {path}:\n{content}")},
            "request_id": fresh,
            "in_reply_to": [reply_to],
        }),
    );
}

fn fresh_request_id() -> JsonRpcId {
    JsonRpcId::String(Ulid::new().to_string())
}

/// Hand-written matcher per scope §PR2 (pi-1 M-3 / pi-2 M2-3).
fn match_read_file(input: &str) -> Option<String> {
    let trimmed = input.trim_start();
    let leading_ws_bytes = input.len() - trimmed.len();
    let folded: String = trimmed
        .bytes()
        .take_while(|b| b.is_ascii())
        .map(|b| b.to_ascii_lowercase() as char)
        .collect();
    let matched_prefix_len = PREFIXES
        .iter()
        .find_map(|p| folded.starts_with(p).then_some(p.len()))?;
    let path_start = leading_ws_bytes + matched_prefix_len;
    let rest = input.get(path_start..)?;
    let candidate = rest.split_whitespace().next()?.trim_end_matches(PATH_PUNCT);
    if candidate.is_empty() {
        None
    } else {
        Some(candidate.to_string())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_simple_read_file() {
        assert_eq!(
            match_read_file("what's in README.md").as_deref(),
            Some("README.md")
        );
    }

    #[test]
    fn strips_trailing_punctuation() {
        assert_eq!(
            match_read_file("what's in README.md?").as_deref(),
            Some("README.md")
        );
    }

    #[test]
    fn case_insensitive_prefix() {
        assert_eq!(
            match_read_file("What Is In notes.txt.").as_deref(),
            Some("notes.txt")
        );
    }

    #[test]
    fn multibyte_path_roundtrip() {
        assert_eq!(
            match_read_file("what's in données.txt").as_deref(),
            Some("données.txt")
        );
    }

    #[test]
    fn no_match_returns_none() {
        assert!(match_read_file("hello").is_none());
        assert!(match_read_file("what's in ").is_none());
    }
}
