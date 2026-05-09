use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::sync::Arc;

use async_trait::async_trait;
use fittings_client::Client;
use fittings_core::{
    context::ServiceContext,
    error::FittingsError,
    message::{JsonRpcId, Request, Response},
    service::Service,
    transport::Connector,
};
use fittings_transport::stdio::StdioTransport;
use serde_json::{json, Value};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::Mutex;

type FixtureTransport = StdioTransport<OwnedReadHalf, OwnedWriteHalf>;

struct OneShotConnector {
    transport: Arc<Mutex<Option<FixtureTransport>>>,
}

impl OneShotConnector {
    fn new(transport: FixtureTransport) -> Self {
        Self {
            transport: Arc::new(Mutex::new(Some(transport))),
        }
    }
}

#[async_trait]
impl Connector for OneShotConnector {
    type Connection = FixtureTransport;

    async fn connect(&self) -> Result<Self::Connection, FittingsError> {
        Ok(self
            .transport
            .lock()
            .await
            .take()
            .expect("OneShotConnector::connect called twice"))
    }
}

struct RespondPeerCallService;

#[async_trait]
impl Service for RespondPeerCallService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        let id = req.id.unwrap_or(JsonRpcId::Null);
        match req.method.as_str() {
            "core.fixture.start" => Ok(Response {
                id,
                result: Value::Null,
                metadata: Default::default(),
            }),
            "core.fixture.echo" => Ok(Response {
                id,
                result: req.params,
                metadata: Default::default(),
            }),
            other => Err(FittingsError::method_not_found(other)),
        }
    }
}

fn main() {
    let mode = std::env::var("RFL_FIXTURE_MODE").unwrap_or_default();
    match mode.as_str() {
        "scaffold_only" => return,
        "respond_peer_call" => {}
        _ => {
            eprintln!("rfl-bus-fixture: unknown mode '{}'", mode);
            std::process::exit(64);
        }
    }

    // Enable I/O + time but not signals: signal-driver init creates a
    // unix socketpair, which lockin's `network_deny` policy blocks.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()
        .expect("build tokio runtime");
    runtime.block_on(run_bus_backed(&mode));
}

async fn run_bus_backed(mode: &str) {
    let fd_str = match std::env::var("RFL_BUS_FD") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("rfl-bus-fixture: RFL_BUS_FD not set");
            std::process::exit(3);
        }
    };
    let fd: RawFd = match fd_str.parse() {
        Ok(n) if n >= 0 => n,
        _ => {
            eprintln!("rfl-bus-fixture: invalid RFL_BUS_FD '{}'", fd_str);
            std::process::exit(3);
        }
    };

    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    let std_stream = std::os::unix::net::UnixStream::from(owned);
    if let Err(err) = std_stream.set_nonblocking(true) {
        eprintln!("rfl-bus-fixture: set_nonblocking failed: {}", err);
        std::process::exit(3);
    }
    let stream = match tokio::net::UnixStream::from_std(std_stream) {
        Ok(s) => s,
        Err(err) => {
            eprintln!(
                "rfl-bus-fixture: tokio UnixStream conversion failed: {}",
                err
            );
            std::process::exit(3);
        }
    };
    let (reader, writer) = stream.into_split();
    let transport = StdioTransport::new(reader, writer, 1 << 20);

    let client = Client::connect(OneShotConnector::new(transport))
        .await
        .expect("client connect");

    match mode {
        "respond_peer_call" => {
            let client = client.with_service(RespondPeerCallService);
            client
                .call("core.fixture.ready", json!({"mode": "respond_peer_call"}))
                .await
                .expect("ready ack");
            std::future::pending::<()>().await;
        }
        _ => unreachable!("mode dispatch already validated"),
    }
}
