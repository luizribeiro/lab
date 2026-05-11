//! `OpenAiProviderHandle` — c33 / scope §OP3 test fixture.
//!
//! Spawns `rfl-openai` against an in-test broker fixture *and* an
//! in-process tokio HTTP stub server. The HTTP stub accepts an
//! arbitrary number of connections, popping one queued `(status,
//! body)` pair per request. The broker side mirrors the
//! `MockProviderHandle` pattern: a `BusPublishService` routes
//! provider notifications back into the broker, and an internal
//! subscription on `provider.openai.**` captures emitted events.

#![allow(dead_code)]

use std::collections::{BTreeMap, VecDeque};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use fittings_core::context::ServiceContext;
use fittings_core::error::FittingsError;
use fittings_core::message::{JsonRpcId, Request, Response};
use fittings_core::service::Service;
use fittings_server::Server;
use fittings_transport::stdio::StdioTransport;
use rafaello_core::broker_acl::{BrokerAcl, PluginAcl};
use rafaello_core::bus::{Broker, BusEvent, InternalSubscription, RegisteredProvider, TaintEntry};
use rafaello_core::lock::CanonicalId;
use serde_json::Value;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use ulid::Ulid;

pub const PROVIDER_ID: &str = "openai";
pub const TOPIC_ID: &str = "openai_local_test";
pub const CANONICAL: &str = "local/test:openai@0.0.0";

const MAX_FRAME_BYTES: usize = 1 << 20;
const EVENT_CAPACITY: usize = 64;
const RECV_TIMEOUT: Duration = Duration::from_secs(5);

struct ConnectionService {
    broker: Broker,
    canonical: CanonicalId,
    tools: Option<Value>,
}

#[async_trait]
impl Service for ConnectionService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        let id = req.id.clone().unwrap_or(JsonRpcId::Null);
        if req.method == "bus.publish" {
            let _ = self
                .broker
                .handle_provider_publish(&self.canonical, &req.params);
            return Ok(Response {
                id,
                result: Value::Null,
                metadata: Default::default(),
            });
        }
        if req.method == "core.tools_list" {
            if let Some(tools) = &self.tools {
                return Ok(Response {
                    id,
                    result: serde_json::json!({"tools": tools.clone()}),
                    metadata: Default::default(),
                });
            }
            return Err(FittingsError::method_not_found(req.method));
        }
        Err(FittingsError::method_not_found(req.method))
    }
}

pub struct HttpStubHandle {
    pub endpoint: String,
    queue: Arc<Mutex<VecDeque<(u16, String)>>>,
    captured: Arc<Mutex<Vec<String>>>,
    _accept_task: JoinHandle<()>,
}

impl HttpStubHandle {
    pub async fn push(&self, status: u16, body: impl Into<String>) {
        self.queue.lock().await.push_back((status, body.into()));
    }
    pub async fn captured_bodies(&self) -> Vec<String> {
        self.captured.lock().await.clone()
    }
}

pub async fn start_http_stub(initial: Vec<(u16, String)>) -> HttpStubHandle {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let queue: Arc<Mutex<VecDeque<(u16, String)>>> =
        Arc::new(Mutex::new(initial.into_iter().collect()));
    let captured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let q = queue.clone();
    let cap = captured.clone();
    let task = tokio::spawn(async move {
        loop {
            let (mut sock, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => return,
            };
            let q = q.clone();
            let cap = cap.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 16 * 1024];
                let mut total = 0usize;
                let mut headers_end = None;
                let mut content_length: usize = 0;
                loop {
                    let n = match tokio::time::timeout(
                        Duration::from_millis(500),
                        sock.read(&mut buf[total..]),
                    )
                    .await
                    {
                        Ok(Ok(n)) if n > 0 => n,
                        _ => break,
                    };
                    total += n;
                    if headers_end.is_none() {
                        if let Some(idx) = find_double_crlf(&buf[..total]) {
                            headers_end = Some(idx + 4);
                            let header_str = std::str::from_utf8(&buf[..idx]).unwrap_or("");
                            for line in header_str.split("\r\n") {
                                if let Some(rest) = line
                                    .strip_prefix("Content-Length: ")
                                    .or_else(|| line.strip_prefix("content-length: "))
                                {
                                    content_length = rest.parse().unwrap_or(0);
                                }
                            }
                        }
                    }
                    if let Some(end) = headers_end {
                        if total >= end + content_length {
                            break;
                        }
                    }
                    if total == buf.len() {
                        break;
                    }
                }
                if let Some(end) = headers_end {
                    let body_slice = &buf[end..total.min(end + content_length)];
                    if let Ok(s) = std::str::from_utf8(body_slice) {
                        cap.lock().await.push(s.to_string());
                    }
                }
                let (status, body) = q
                    .lock()
                    .await
                    .pop_front()
                    .unwrap_or((500, "{\"error\":\"stub-queue-empty\"}".to_string()));
                let reason = match status {
                    401 => "Unauthorized",
                    500 => "Internal Server Error",
                    _ => "OK",
                };
                let resp = format!(
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = sock.write_all(resp.as_bytes()).await;
                let _ = sock.flush().await;
            });
        }
    });
    HttpStubHandle {
        endpoint: format!("http://127.0.0.1:{port}"),
        queue,
        captured,
        _accept_task: task,
    }
}

fn find_double_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

pub struct OpenAiProviderHandle {
    pub broker: Broker,
    pub canonical: CanonicalId,
    pub http: HttpStubHandle,
    events_rx: mpsc::Receiver<BusEvent>,
    _subscription: InternalSubscription,
    _registration: RegisteredProvider,
    child: Child,
    _server_join: JoinHandle<()>,
    _tempdir: TempDir,
}

impl OpenAiProviderHandle {
    pub async fn launch(http: HttpStubHandle) -> Self {
        Self::launch_with_tools(http, Some(Value::Array(Vec::new()))).await
    }

    pub async fn launch_with_tools(http: HttpStubHandle, tools: Option<Value>) -> Self {
        let canonical = CanonicalId::parse(CANONICAL).expect("canonical id");
        let acl = build_acl(&canonical);
        let broker = Broker::new(acl).expect("broker");

        let (core_fd, child_fd) = nix::sys::socket::socketpair(
            nix::sys::socket::AddressFamily::Unix,
            nix::sys::socket::SockType::Stream,
            None,
            nix::sys::socket::SockFlag::empty(),
        )
        .expect("socketpair");

        let child_raw = child_fd.as_raw_fd();

        let std_core = std::os::unix::net::UnixStream::from(core_fd);
        std_core
            .set_nonblocking(true)
            .expect("set core non-blocking");
        let tokio_core = tokio::net::UnixStream::from_std(std_core).expect("tokio UnixStream");
        let (reader, writer) = tokio_core.into_split();
        let transport = StdioTransport::new(reader, writer, MAX_FRAME_BYTES);

        let service = ConnectionService {
            broker: broker.clone(),
            canonical: canonical.clone(),
            tools,
        };
        let server = Server::new(service, transport);
        let peer = server.peer();
        let _registration = broker
            .register_provider(canonical.clone(), peer)
            .expect("register provider");
        let _server_join = tokio::spawn(async move {
            let _ = server.serve().await;
        });

        let (rx, _subscription) =
            broker.subscribe_internal(vec![format!("provider.{PROVIDER_ID}.**")], EVENT_CAPACITY);

        let tempdir = TempDir::new().expect("tempdir");
        let project_root = tempdir.path().to_path_buf();
        let private_state = tempdir.path().join("priv");
        std::fs::create_dir_all(&private_state).expect("priv dir");

        let bin = PathBuf::from(env!("CARGO_BIN_EXE_rfl-openai"));
        let mut cmd = Command::new(&bin);
        cmd.env_clear();
        cmd.env("RFL_BUS_FD", child_raw.to_string());
        cmd.env("RFL_PROVIDER_ID", PROVIDER_ID);
        cmd.env("RFL_PLUGIN", CANONICAL);
        cmd.env("RFL_TOPIC_ID", TOPIC_ID);
        cmd.env("RFL_PROJECT_ROOT", &project_root);
        cmd.env("RFL_PRIVATE_STATE_DIR", &private_state);
        cmd.env("RFL_OPENAI_MODEL", "vllm/qwen3.6-27b");
        cmd.env("RFL_OPENAI_ENDPOINT_URL", &http.endpoint);
        cmd.env("RFL_OPENAI_API_KEY_ENV", "RFL_OPENAI_API_KEY");
        cmd.env("RFL_OPENAI_API_KEY", "sk-test");
        if let Ok(rust_log) = std::env::var("RUST_LOG") {
            cmd.env("RUST_LOG", rust_log);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        let child = cmd.spawn().expect("spawn rfl-openai");

        drop(child_fd);

        Self {
            broker,
            canonical,
            http,
            events_rx: rx,
            _subscription,
            _registration,
            child,
            _server_join,
            _tempdir: tempdir,
        }
    }

    pub fn publish_user_message(&self, text: &str) -> JsonRpcId {
        let id = JsonRpcId::String(Ulid::new().to_string());
        self.broker
            .publish_core_with_taint(
                "core.session.user_message",
                serde_json::json!({"text": text}),
                Some(id.clone()),
                None,
                None,
                None,
            )
            .expect("publish core.session.user_message");
        id
    }

    pub fn inject_tool_result(&self, reply_to: JsonRpcId, content: &str) -> JsonRpcId {
        let id = JsonRpcId::String(Ulid::new().to_string());
        self.broker
            .publish_core_with_taint(
                "core.session.tool_result",
                serde_json::json!({"ok": true, "content": content}),
                Some(id.clone()),
                Some(vec![reply_to]),
                Some(vec![TaintEntry {
                    source: "user".to_string(),
                    detail: None,
                }]),
                None,
            )
            .expect("publish core.session.tool_result");
        id
    }

    pub async fn wait_exit(&mut self) -> std::process::ExitStatus {
        tokio::time::timeout(RECV_TIMEOUT, self.child.wait())
            .await
            .expect("child wait timed out")
            .expect("child wait")
    }

    pub async fn recv_event(&mut self) -> BusEvent {
        match tokio::time::timeout(RECV_TIMEOUT, self.events_rx.recv()).await {
            Ok(Some(event)) => event,
            Ok(None) => panic!("event channel closed"),
            Err(_) => panic!("timed out waiting for event"),
        }
    }
}

impl Drop for OpenAiProviderHandle {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

fn build_acl(canonical: &CanonicalId) -> BrokerAcl {
    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: TOPIC_ID.to_string(),
            publish_topics: vec![
                format!("provider.{PROVIDER_ID}.tool_request"),
                format!("provider.{PROVIDER_ID}.assistant_message"),
            ],
            subscribe_patterns: vec![
                "core.session.user_message".to_string(),
                "core.session.tool_result".to_string(),
            ],
            auto_subscribes: vec![],
            provider_id: Some(PROVIDER_ID.to_string()),
        },
    );
    BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    }
}

pub fn payload_text(event: &BusEvent) -> &str {
    event
        .payload
        .get("text")
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

pub fn payload_tool(event: &BusEvent) -> &str {
    event
        .payload
        .get("tool")
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

pub fn payload_args(event: &BusEvent) -> &Value {
    event.payload.get("args").unwrap_or(&Value::Null)
}

pub fn topic_for(suffix: &str) -> String {
    format!("provider.{PROVIDER_ID}.{suffix}")
}
