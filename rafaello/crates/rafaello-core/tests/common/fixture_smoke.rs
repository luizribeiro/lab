//! c16 smoke harness: spawn `rfl-bus-fixture` directly with a
//! socketpair as `RFL_BUS_FD` and a fittings `Server` on the
//! parent side that records every inbound request / notification.
//!
//! Intentionally bypasses `PluginSupervisor` + `lockin::Sandbox`
//! so the new m3 fixture modes can be exercised without the m1/m2
//! plumbing each test would otherwise have to materialise.

#![cfg(all(feature = "test-fixture", target_os = "linux"))]
#![allow(dead_code)]

use std::os::fd::{AsRawFd, IntoRawFd, OwnedFd};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use fittings_core::{
    context::ServiceContext,
    error::FittingsError,
    message::{JsonRpcId, Request, Response},
    service::Service,
};
use fittings_server::Server;
use fittings_transport::stdio::StdioTransport;
use serde_json::Value;
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct Recorded {
    pub method: String,
    pub params: Value,
    pub is_notification: bool,
}

struct RecorderService {
    tx: mpsc::UnboundedSender<Recorded>,
}

#[async_trait]
impl Service for RecorderService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        let is_notification = req.id.is_none();
        let id = req.id.clone().unwrap_or(JsonRpcId::Null);
        let _ = self.tx.send(Recorded {
            method: req.method.clone(),
            params: req.params.clone(),
            is_notification,
        });
        Ok(Response {
            id,
            result: Value::Null,
            metadata: Default::default(),
        })
    }
}

pub struct FixtureSmoke {
    pub child: Child,
    pub events: mpsc::UnboundedReceiver<Recorded>,
    _server: tokio::task::JoinHandle<()>,
}

pub fn fixture_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_rfl-bus-fixture"))
}

/// Spawn the fixture binary with mode + envs. Caller-supplied
/// envs override the defaults set here.
pub fn spawn_fixture_with_bus(mode: &str, envs: &[(&str, &str)]) -> FixtureSmoke {
    let (parent_stream, child_fd) = make_inheritable_socketpair();
    let child_raw = child_fd.as_raw_fd();

    let mut cmd = Command::new(fixture_bin());
    cmd.env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        cmd.env("PATH", path);
    }
    cmd.env("RFL_FIXTURE_MODE", mode)
        .env("RFL_BUS_FD", child_raw.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    for (k, v) in envs {
        cmd.env(*k, *v);
    }

    let child = cmd.spawn().expect("spawn rfl-bus-fixture");
    drop(child_fd);

    let (reader, writer) = parent_stream.into_split();
    let transport = StdioTransport::new(reader, writer, 1 << 20);
    let (tx, rx) = mpsc::unbounded_channel::<Recorded>();
    let server = Server::new(RecorderService { tx }, transport);
    let server_handle = tokio::spawn(async move {
        let _ = server.serve().await;
    });

    FixtureSmoke {
        child,
        events: rx,
        _server: server_handle,
    }
}

/// Spawn the fixture binary without setting up a bus (modes that
/// don't adopt RFL_BUS_FD).
pub fn spawn_fixture_no_bus(mode: &str, extra_args: &[&str], envs: &[(&str, &str)]) -> Child {
    let mut cmd = Command::new(fixture_bin());
    cmd.env_clear();
    if let Some(path) = std::env::var_os("PATH") {
        cmd.env("PATH", path);
    }
    cmd.env("RFL_FIXTURE_MODE", mode)
        .args(extra_args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    for (k, v) in envs {
        cmd.env(*k, *v);
    }
    cmd.spawn().expect("spawn rfl-bus-fixture")
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

pub async fn wait_for_method(
    events: &mut mpsc::UnboundedReceiver<Recorded>,
    method: &str,
    timeout: Duration,
) -> Recorded {
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            panic!("timed out waiting for inbound `{method}`");
        }
        let recorded = tokio::time::timeout(remaining, events.recv())
            .await
            .unwrap_or_else(|_| panic!("timed out waiting for inbound `{method}`"))
            .expect("recorder channel closed");
        if recorded.method == method {
            return recorded;
        }
    }
}

/// Avoid "unused" warnings for the helper from tests that only
/// touch a subset of the surface.
pub fn _silence_unused_into_raw(fd: OwnedFd) -> i32 {
    fd.into_raw_fd()
}
