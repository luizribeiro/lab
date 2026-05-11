//! `ReadFileToolHandle` — c23 / scope §H2.
//!
//! Spawns `rfl-readfile` against an in-test broker fixture. The broker
//! side runs a `bus.publish` service that routes the plugin's
//! `plugin.<topic-id>.tool_result` publishes back through
//! `Broker::handle_plugin_publish`. An internal subscriber captures
//! emitted events for assertions; `publish_tool_request` drives a
//! `plugin.<topic-id>.tool_request` event into the subprocess via
//! `Broker::publish_for_tool_dispatch`.

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
use rafaello_core::bus::{Broker, BusEvent, InternalSubscription, RegisteredPlugin};
use rafaello_core::lock::CanonicalId;
use serde_json::Value;
use tempfile::TempDir;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use ulid::Ulid;

pub const TOPIC_ID: &str = "readfile_local_test";
pub const CANONICAL: &str = "local/test:readfile@0.1.0";

const MAX_FRAME_BYTES: usize = 1 << 20;
const EVENT_CAPACITY: usize = 64;
const RECV_TIMEOUT: Duration = Duration::from_secs(10);
const SANDBOX_BUS_FD: i32 = 3;

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
                .handle_plugin_publish(&self.canonical, &req.params);
            return Ok(Response {
                id,
                result: Value::Null,
                metadata: Default::default(),
            });
        }
        Err(FittingsError::method_not_found(req.method))
    }
}

pub struct LaunchOpts {
    pub project_root: PathBuf,
    pub bypass_guard: bool,
    /// When `Some`, spawn under `lockin::Sandbox` with the given
    /// `read_dirs` grant. Linux-only; tests that set this must
    /// `#[cfg(target_os = "linux")]`-gate.
    pub sandbox_read_dirs: Option<Vec<PathBuf>>,
}

pub enum ChildHandle {
    Plain(Child),
    #[cfg(target_os = "linux")]
    Sandboxed(Box<lockin::tokio::SandboxedChild>),
}

impl ChildHandle {
    fn start_kill(&mut self) {
        match self {
            ChildHandle::Plain(c) => {
                let _ = c.start_kill();
            }
            #[cfg(target_os = "linux")]
            ChildHandle::Sandboxed(c) => {
                let _ = c.start_kill();
            }
        }
    }
}

pub struct ReadFileToolHandle {
    pub broker: Broker,
    pub canonical: CanonicalId,
    events_rx: mpsc::Receiver<BusEvent>,
    _subscription: InternalSubscription,
    _registration: RegisteredPlugin,
    child: ChildHandle,
    _server_join: JoinHandle<()>,
    _tempdir: TempDir,
}

impl ReadFileToolHandle {
    pub async fn launch(opts: LaunchOpts) -> Self {
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
            .register_plugin(canonical.clone(), peer)
            .expect("register plugin");
        let _server_join = tokio::spawn(async move {
            let _ = server.serve().await;
        });

        let (rx, _subscription) = broker.subscribe_internal(
            vec![format!("plugin.{TOPIC_ID}.tool_result")],
            EVENT_CAPACITY,
        );

        let tempdir = TempDir::new().expect("tempdir");
        let private_state = tempdir.path().join("priv");
        std::fs::create_dir_all(&private_state).expect("priv dir");

        let bin = PathBuf::from(env!("CARGO_BIN_EXE_rfl-readfile"));

        let child = match &opts.sandbox_read_dirs {
            None => {
                let child_raw = child_fd.as_raw_fd();
                let mut cmd = Command::new(&bin);
                cmd.env_clear();
                cmd.env("RFL_BUS_FD", child_raw.to_string());
                cmd.env("RFL_PLUGIN", CANONICAL);
                cmd.env("RFL_TOPIC_ID", TOPIC_ID);
                cmd.env("RFL_PROJECT_ROOT", &opts.project_root);
                cmd.env("RFL_PRIVATE_STATE_DIR", &private_state);
                if opts.bypass_guard {
                    cmd.env("RFL_READFILE_TEST_BYPASS_GUARD", "1");
                }
                if let Ok(rust_log) = std::env::var("RUST_LOG") {
                    cmd.env("RUST_LOG", rust_log);
                }
                cmd.stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit());
                let child = cmd.spawn().expect("spawn rfl-readfile");
                drop(child_fd);
                ChildHandle::Plain(child)
            }
            #[cfg(target_os = "linux")]
            Some(read_dirs) => {
                let mut builder = lockin::Sandbox::builder();
                for p in read_dirs {
                    builder = builder.read_dir(p);
                }
                builder = builder.read_dir(&private_state);
                builder = builder.exec_path(&bin);
                for d in runtime_exec_dirs() {
                    builder = builder.exec_dir(d);
                }
                builder = builder.network_allow_all().disable_core_dumps();
                builder = builder.inherit_fd_as(child_fd, SANDBOX_BUS_FD);
                let mut cmd = builder.tokio_command(&bin).expect("lockin tokio_command");
                cmd.env_clear();
                cmd.env("RFL_BUS_FD", SANDBOX_BUS_FD.to_string());
                cmd.env("RFL_PLUGIN", CANONICAL);
                cmd.env("RFL_TOPIC_ID", TOPIC_ID);
                cmd.env("RFL_PROJECT_ROOT", &opts.project_root);
                cmd.env("RFL_PRIVATE_STATE_DIR", &private_state);
                if opts.bypass_guard {
                    cmd.env("RFL_READFILE_TEST_BYPASS_GUARD", "1");
                }
                if let Ok(rust_log) = std::env::var("RUST_LOG") {
                    cmd.env("RUST_LOG", rust_log);
                }
                cmd.stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::inherit());
                let child = cmd.spawn().expect("spawn rfl-readfile (sandboxed)");
                ChildHandle::Sandboxed(Box::new(child))
            }
            #[cfg(not(target_os = "linux"))]
            Some(_) => panic!("sandbox path only supported on linux"),
        };

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

    pub fn publish_tool_request(&self, path: &str) -> JsonRpcId {
        let id = JsonRpcId::String(Ulid::new().to_string());
        self.broker
            .publish_for_tool_dispatch(
                &self.canonical,
                serde_json::json!({
                    "tool": "read-file",
                    "args": {"path": path},
                }),
                id.clone(),
                None,
                None,
            )
            .expect("publish_for_tool_dispatch");
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

impl Drop for ReadFileToolHandle {
    fn drop(&mut self) {
        self.child.start_kill();
    }
}

fn build_acl(canonical: &CanonicalId) -> BrokerAcl {
    let mut plugins = BTreeMap::new();
    plugins.insert(
        canonical.clone(),
        PluginAcl {
            topic_id: TOPIC_ID.to_string(),
            publish_topics: vec![format!("plugin.{TOPIC_ID}.tool_result")],
            subscribe_patterns: vec![],
            auto_subscribes: vec![format!("plugin.{TOPIC_ID}.tool_request")],
            provider_id: None,
        },
    );
    BrokerAcl {
        plugins,
        tool_routes: BTreeMap::new(),
        frontends: BTreeMap::new(),
    }
}

#[cfg(target_os = "linux")]
fn runtime_exec_dirs() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(val) = std::env::var_os("LOCKIN_TEST_EXEC_DIRS") {
        for d in std::env::split_paths(&val) {
            if d.is_absolute() {
                out.push(d);
            }
        }
    }
    if out.is_empty() {
        out.push(PathBuf::from("/nix/store"));
    }
    out
}

pub fn payload_ok(event: &BusEvent) -> bool {
    event
        .payload
        .get("ok")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

pub fn payload_content(event: &BusEvent) -> &str {
    event
        .payload
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

pub fn payload_error(event: &BusEvent) -> &str {
    event
        .payload
        .get("error")
        .and_then(|v| v.as_str())
        .unwrap_or("")
}
