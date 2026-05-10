//! Shared harness for `rfl-tui` integration tests (c25).
//!
//! Spawns the `rfl-tui` binary with a socketpair as `RFL_BUS_FD`, attaches
//! a fittings `Server` on the parent end, captures stderr line-by-line, and
//! exposes hooks for the per-test [`ParentService`] (the inbound dispatch
//! the parent server presents to the child).

#![cfg(target_os = "linux")]
#![allow(dead_code)]

use std::os::fd::{AsRawFd, OwnedFd};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use fittings_core::{
    context::{PeerHandle, ServiceContext},
    error::FittingsError,
    message::{JsonRpcId, Request, Response},
    service::Service,
};
use fittings_server::Server;
use fittings_transport::stdio::StdioTransport;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStderr, Command};
use tokio::sync::mpsc;

pub fn tui_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rfl-tui"))
}

#[derive(Debug, Clone)]
pub struct Recorded {
    pub method: String,
    pub params: Value,
    pub is_notification: bool,
}

pub struct RecordingService {
    tx: mpsc::UnboundedSender<Recorded>,
}

impl RecordingService {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<Recorded>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { tx }, rx)
    }
}

#[async_trait]
impl Service for RecordingService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        let is_notification = req.id.is_none();
        let id = req.id.clone().unwrap_or(JsonRpcId::Null);
        let _ = self.tx.send(Recorded {
            method: req.method,
            params: req.params,
            is_notification,
        });
        Ok(Response {
            id,
            result: Value::Null,
            metadata: Default::default(),
        })
    }
}

/// Service that delegates to an inner [`Service`] but, on the first inbound
/// `frontend.ready`, runs `on_ready(peer)` synchronously before responding.
pub struct OnReadyService<F, S>
where
    F: Fn(&PeerHandle) + Send + Sync + 'static,
    S: Service + Send + Sync + 'static,
{
    inner: S,
    on_ready: F,
    fired: std::sync::atomic::AtomicBool,
}

impl<F, S> OnReadyService<F, S>
where
    F: Fn(&PeerHandle) + Send + Sync + 'static,
    S: Service + Send + Sync + 'static,
{
    pub fn new(inner: S, on_ready: F) -> Self {
        Self {
            inner,
            on_ready,
            fired: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl<F, S> Service for OnReadyService<F, S>
where
    F: Fn(&PeerHandle) + Send + Sync + 'static,
    S: Service + Send + Sync + 'static,
{
    async fn call(&self, req: Request, ctx: ServiceContext) -> Result<Response, FittingsError> {
        if req.method == "frontend.ready"
            && !self.fired.swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            (self.on_ready)(ctx.peer());
        }
        self.inner.call(req, ctx).await
    }
}

pub struct TuiHarness {
    pub child: Child,
    pub stderr_lines: mpsc::UnboundedReceiver<String>,
    pub parent_peer: PeerHandle,
    pub project_root: tempfile::TempDir,
    _server: tokio::task::JoinHandle<()>,
    _stderr: tokio::task::JoinHandle<()>,
}

pub struct SpawnOpts {
    pub test_mode: bool,
    pub max_lifetime: Option<u64>,
    pub ready_delay_ms: Option<u64>,
}

impl Default for SpawnOpts {
    fn default() -> Self {
        Self {
            test_mode: true,
            max_lifetime: Some(2),
            ready_delay_ms: None,
        }
    }
}

pub fn spawn_tui<S: Service + Send + Sync + 'static>(opts: SpawnOpts, service: S) -> TuiHarness {
    let project_root = tempfile::tempdir().expect("tempdir for project root");
    let (parent_stream, child_fd) = make_inheritable_socketpair();
    let child_raw = child_fd.as_raw_fd();

    let mut cmd = Command::new(tui_bin());
    cmd.env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        cmd.env("PATH", path);
    }
    cmd.env("RFL_BUS_FD", child_raw.to_string())
        .env("RFL_PROJECT_ROOT", project_root.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if opts.test_mode {
        cmd.env("RFL_TUI_TEST_MODE", "1");
    }
    if let Some(s) = opts.max_lifetime {
        cmd.env("RFL_TUI_MAX_LIFETIME", s.to_string());
    }
    if let Some(ms) = opts.ready_delay_ms {
        cmd.env("RFL_TUI_READY_DELAY_MS", ms.to_string());
    }

    let mut child = cmd.spawn().expect("spawn rfl-tui");
    drop(child_fd);

    let stderr = child.stderr.take().expect("child stderr piped");
    let (line_tx, line_rx) = mpsc::unbounded_channel::<String>();
    let stderr_handle = tokio::spawn(forward_stderr(stderr, line_tx));

    let (reader, writer) = parent_stream.into_split();
    let transport = StdioTransport::new(reader, writer, 1 << 20);
    let server = Server::new(service, transport);
    let parent_peer = server.peer();
    let server_handle = tokio::spawn(async move {
        let _ = server.serve().await;
    });

    TuiHarness {
        child,
        stderr_lines: line_rx,
        parent_peer,
        project_root,
        _server: server_handle,
        _stderr: stderr_handle,
    }
}

async fn forward_stderr(stderr: ChildStderr, tx: mpsc::UnboundedSender<String>) {
    let mut reader = BufReader::new(stderr).lines();
    while let Ok(Some(line)) = reader.next_line().await {
        if tx.send(line).is_err() {
            break;
        }
    }
}

fn make_inheritable_socketpair() -> (tokio::net::UnixStream, OwnedFd) {
    let (a, b) = nix::sys::socket::socketpair(
        nix::sys::socket::AddressFamily::Unix,
        nix::sys::socket::SockType::Stream,
        None,
        nix::sys::socket::SockFlag::empty(),
    )
    .expect("socketpair");

    nix::fcntl::fcntl(
        a.as_raw_fd(),
        nix::fcntl::FcntlArg::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC),
    )
    .expect("cloexec parent fd");

    let std_stream = std::os::unix::net::UnixStream::from(a);
    std_stream
        .set_nonblocking(true)
        .expect("parent set_nonblocking");
    let parent = tokio::net::UnixStream::from_std(std_stream).expect("tokio UnixStream");
    (parent, b)
}

pub async fn wait_for_stderr_line(
    rx: &mut mpsc::UnboundedReceiver<String>,
    needle: &str,
    timeout: Duration,
) -> String {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!("timed out waiting for stderr line containing `{needle}`");
        }
        let line = tokio::time::timeout(remaining, rx.recv())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for stderr line containing `{needle}`"))
            .expect("stderr channel closed before line found");
        if line.contains(needle) {
            return line;
        }
    }
}

pub async fn wait_for_method(
    rx: &mut mpsc::UnboundedReceiver<Recorded>,
    method: &str,
    timeout: Duration,
) -> Recorded {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!("timed out waiting for inbound `{method}`");
        }
        let recorded = tokio::time::timeout(remaining, rx.recv())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for inbound `{method}`"))
            .expect("recorder channel closed");
        if recorded.method == method {
            return recorded;
        }
    }
}

pub async fn expect_clean_exit(child: &mut Child, timeout: Duration) {
    let status = tokio::time::timeout(timeout, child.wait())
        .await
        .expect("child did not exit within timeout")
        .expect("child wait failed");
    assert_eq!(status.code(), Some(0), "expected clean exit (status=0)");
}

pub fn publish_bus_event(peer: &PeerHandle, topic: &str) {
    peer.notify(
        "bus.event",
        serde_json::json!({
            "topic": topic,
            "payload": {},
            "publisher": { "kind": "core" },
        }),
    )
    .expect("publish bus.event");
}
