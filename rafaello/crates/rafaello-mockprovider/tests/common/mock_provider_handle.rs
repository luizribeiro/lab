//! `MockProviderHandle` — c21 / scope §H1.
//!
//! Spawns `rfl-mockprovider` against an in-test broker fixture. The
//! broker side runs a fittings server with a `bus.publish` service
//! that routes provider notifications back to
//! `broker.handle_provider_publish`. Internal subscribers capture
//! emitted events for assertions; `publish_user_message` /
//! `inject_tool_result` drive `core.session.*` traffic into the
//! subprocess via the broker's fan-out.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::process::Stdio;
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
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use ulid::Ulid;

pub const PROVIDER_ID: &str = "mock";
pub const TOPIC_ID: &str = "mockprov_local_test";
pub const CANONICAL: &str = "local/test:mockprov@0.1.0";

const MAX_FRAME_BYTES: usize = 1 << 20;
const EVENT_CAPACITY: usize = 64;
const RECV_TIMEOUT: Duration = Duration::from_secs(5);

struct BusPublishService {
    broker: Broker,
    canonical: CanonicalId,
}

#[async_trait]
impl Service for BusPublishService {
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
        Err(FittingsError::method_not_found(req.method))
    }
}

pub struct MockProviderHandle {
    pub broker: Broker,
    pub canonical: CanonicalId,
    events_rx: mpsc::Receiver<BusEvent>,
    _subscription: InternalSubscription,
    _registration: RegisteredProvider,
    child: Child,
    _server_join: JoinHandle<()>,
    _tempdir: TempDir,
}

impl MockProviderHandle {
    pub async fn launch() -> Self {
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

        let service = BusPublishService {
            broker: broker.clone(),
            canonical: canonical.clone(),
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

        let bin = PathBuf::from(env!("CARGO_BIN_EXE_rfl-mockprovider"));
        let mut cmd = Command::new(&bin);
        cmd.env_clear();
        cmd.env("RFL_BUS_FD", child_raw.to_string());
        cmd.env("RFL_PROVIDER_ID", PROVIDER_ID);
        cmd.env("RFL_PLUGIN", CANONICAL);
        cmd.env("RFL_TOPIC_ID", TOPIC_ID);
        cmd.env("RFL_PROJECT_ROOT", &project_root);
        cmd.env("RFL_PRIVATE_STATE_DIR", &private_state);
        if let Ok(rust_log) = std::env::var("RUST_LOG") {
            cmd.env("RUST_LOG", rust_log);
        }
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit());
        let child = cmd.spawn().expect("spawn rfl-mockprovider");

        drop(child_fd);

        Self {
            broker,
            canonical,
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

    pub async fn recv_event(&mut self) -> BusEvent {
        match tokio::time::timeout(RECV_TIMEOUT, self.events_rx.recv()).await {
            Ok(Some(event)) => event,
            Ok(None) => panic!("event channel closed"),
            Err(_) => panic!("timed out waiting for event"),
        }
    }
}

impl Drop for MockProviderHandle {
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

pub fn payload_path(event: &BusEvent) -> &str {
    event
        .payload
        .get("args")
        .and_then(|a| a.get("path"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
}
