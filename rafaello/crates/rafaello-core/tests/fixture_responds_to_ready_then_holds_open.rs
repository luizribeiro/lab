#![cfg(all(feature = "test-fixture", target_os = "linux"))]

use std::os::fd::{IntoRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::Command;
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
use serde_json::{json, Value};
use tokio::sync::mpsc;

struct ReadyCaptureService {
    ready_tx: mpsc::UnboundedSender<Value>,
}

#[async_trait]
impl Service for ReadyCaptureService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        let id = req.id.unwrap_or(JsonRpcId::Null);
        match req.method.as_str() {
            "core.fixture.ready" => {
                let _ = self.ready_tx.send(req.params);
                Ok(Response {
                    id,
                    result: Value::Null,
                    metadata: Default::default(),
                })
            }
            other => Err(FittingsError::method_not_found(other)),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn fixture_responds_to_ready_then_holds_open() {
    let (parent_fd, child_fd) = nix::sys::socket::socketpair(
        nix::sys::socket::AddressFamily::Unix,
        nix::sys::socket::SockType::Stream,
        None,
        nix::sys::socket::SockFlag::SOCK_CLOEXEC,
    )
    .expect("socketpair");

    let parent_owned: OwnedFd = parent_fd;
    let parent_std = std::os::unix::net::UnixStream::from(parent_owned);
    parent_std.set_nonblocking(true).expect("set_nonblocking");
    let parent_stream = tokio::net::UnixStream::from_std(parent_std).expect("tokio stream");
    let (reader, writer) = parent_stream.into_split();
    let transport = StdioTransport::new(reader, writer, 1 << 20);

    let (ready_tx, mut ready_rx) = mpsc::unbounded_channel::<Value>();
    let server = Server::new(ReadyCaptureService { ready_tx }, transport);
    let peer = server.peer();
    let serve_task = tokio::spawn(async move {
        let _ = server.serve().await;
    });

    let path = env!("CARGO_BIN_EXE_rfl-bus-fixture");
    let child_raw: RawFd = child_fd.into_raw_fd();
    let mut cmd = Command::new(path);
    cmd.env("RFL_FIXTURE_MODE", "respond_peer_call")
        .env("RFL_BUS_FD", "3");
    unsafe {
        cmd.pre_exec(move || {
            nix::unistd::dup2(child_raw, 3).map_err(std::io::Error::from)?;
            Ok(())
        });
    }
    let mut child = cmd.spawn().expect("spawn fixture");
    let _ = nix::unistd::close(child_raw);

    let ready_params = tokio::time::timeout(Duration::from_secs(5), ready_rx.recv())
        .await
        .expect("ready timed out")
        .expect("ready channel closed");
    assert_eq!(ready_params, json!({"mode": "respond_peer_call"}));

    let start_resp = tokio::time::timeout(
        Duration::from_secs(5),
        peer.call("core.fixture.start", json!({})),
    )
    .await
    .expect("start timed out")
    .expect("start failed");
    assert_eq!(start_resp, Value::Null);

    let echo_resp = tokio::time::timeout(
        Duration::from_secs(5),
        peer.call("core.fixture.echo", json!({"x": 1})),
    )
    .await
    .expect("echo timed out")
    .expect("echo failed");
    assert_eq!(echo_resp, json!({"x": 1}));

    let pid = child.id() as i32;
    nix::sys::signal::kill(
        nix::unistd::Pid::from_raw(pid),
        nix::sys::signal::Signal::SIGTERM,
    )
    .expect("sigterm");

    let status = tokio::task::spawn_blocking(move || child.wait())
        .await
        .expect("join wait")
        .expect("wait child");
    assert!(
        !status.success(),
        "expected non-zero exit on SIGTERM, got {:?}",
        status
    );

    serve_task.abort();
    let _ = serve_task.await;
}
