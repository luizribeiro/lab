//! c30 acceptance: stdio and tcp transports carry simultaneous bidirectional
//! traffic (100 `peer.call`s in each direction) without ordering bugs or
//! orphan responses.
//!
//! scope §T1: tests-only regression after Group 3's bidirectional `PeerHandle`
//! lands. Each transport flavour drives the same `run_bidirectional_load`
//! body so any divergence between transports surfaces here.

use std::sync::Arc;

use fittings::{
    async_trait::async_trait,
    core::message::{JsonRpcId, Request, Response},
    core::transport::Connector,
    transport::{
        stdio::StdioTransport,
        tcp::{TcpConnectionListener, TcpConnector},
    },
    Client, FittingsError, Listener, Server, Service, ServiceContext, Transport,
};
use serde_json::json;
use tokio::{
    io::{ReadHalf, WriteHalf},
    sync::Mutex,
    task::JoinSet,
    time::{timeout, Duration},
};

const CALLS_PER_DIRECTION: u64 = 100;
const TEST_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_FRAME_BYTES: usize = 64 * 1024;
const STDIO_PIPE_BUF: usize = 64 * 1024;

/// Echoes the inbound `n` parameter back so the caller can verify that the
/// response correlates to the right request id (no cross-talk between the
/// two directions).
struct EchoService {
    side: &'static str,
}

#[async_trait]
impl Service for EchoService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        Ok(Response {
            id: req.id.unwrap_or(JsonRpcId::Null),
            result: json!({ "side": self.side, "method": req.method, "params": req.params }),
            metadata: Default::default(),
        })
    }
}

struct OneShotConnector<T: Transport + 'static> {
    transport: Arc<Mutex<Option<T>>>,
}

impl<T: Transport + 'static> OneShotConnector<T> {
    fn new(transport: T) -> Self {
        Self {
            transport: Arc::new(Mutex::new(Some(transport))),
        }
    }
}

#[async_trait]
impl<T: Transport + Send + 'static> Connector for OneShotConnector<T> {
    type Connection = T;

    async fn connect(&self) -> Result<Self::Connection, FittingsError> {
        self.transport
            .lock()
            .await
            .take()
            .ok_or_else(|| FittingsError::internal("connector already used"))
    }
}

async fn run_bidirectional_load<T, C>(server: Server<EchoService, T>, client: Client<C>)
where
    T: Transport + Sync + 'static,
    C: Connector + Send + Sync + 'static,
{
    let server_peer = server.peer();
    let client_peer = client.peer();
    let serve = tokio::spawn(server.serve());

    let mut s_to_c = JoinSet::new();
    let mut c_to_s = JoinSet::new();

    for n in 0..CALLS_PER_DIRECTION {
        let peer = server_peer.clone();
        s_to_c.spawn(async move {
            let result = peer
                .call("server-asks", json!({ "n": n }))
                .await
                .expect("server-initiated call ok");
            (n, result)
        });

        let peer = client_peer.clone();
        c_to_s.spawn(async move {
            let result = peer
                .call("client-asks", json!({ "n": n }))
                .await
                .expect("client-initiated call ok");
            (n, result)
        });
    }

    let mut s_seen = vec![false; CALLS_PER_DIRECTION as usize];
    let mut c_seen = vec![false; CALLS_PER_DIRECTION as usize];

    while let Some(joined) = s_to_c.join_next().await {
        let (n, result) = joined.expect("server-side join");
        assert_eq!(
            result,
            json!({ "side": "client", "method": "server-asks", "params": { "n": n } }),
            "server->client response correlated to wrong request"
        );
        assert!(
            !s_seen[n as usize],
            "duplicate response for server call {n}"
        );
        s_seen[n as usize] = true;
    }

    while let Some(joined) = c_to_s.join_next().await {
        let (n, result) = joined.expect("client-side join");
        assert_eq!(
            result,
            json!({ "side": "server", "method": "client-asks", "params": { "n": n } }),
            "client->server response correlated to wrong request"
        );
        assert!(
            !c_seen[n as usize],
            "duplicate response for client call {n}"
        );
        c_seen[n as usize] = true;
    }

    assert!(
        s_seen.iter().all(|seen| *seen),
        "missing server->client responses"
    );
    assert!(
        c_seen.iter().all(|seen| *seen),
        "missing client->server responses"
    );

    drop(client);
    let serve_result = serve.await.expect("server task join");
    assert!(
        serve_result.is_ok(),
        "server should exit cleanly: {serve_result:?}"
    );
}

type DuplexStdio =
    StdioTransport<ReadHalf<tokio::io::DuplexStream>, WriteHalf<tokio::io::DuplexStream>>;

fn stdio_pair() -> (DuplexStdio, DuplexStdio) {
    let (client_side, server_side) = tokio::io::duplex(STDIO_PIPE_BUF);
    let (sr, sw) = tokio::io::split(server_side);
    let (cr, cw) = tokio::io::split(client_side);
    (
        StdioTransport::new(sr, sw, MAX_FRAME_BYTES),
        StdioTransport::new(cr, cw, MAX_FRAME_BYTES),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stdio_carries_bidirectional_traffic() {
    let (server_transport, client_transport) = stdio_pair();
    let server = Server::new(EchoService { side: "server" }, server_transport)
        .with_max_in_flight(CALLS_PER_DIRECTION as usize * 2);
    let client = Client::connect(OneShotConnector::new(client_transport))
        .await
        .expect("client connects")
        .with_service(EchoService { side: "client" });

    timeout(TEST_TIMEOUT, run_bidirectional_load(server, client))
        .await
        .expect("stdio bidirectional load finishes within timeout");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tcp_carries_bidirectional_traffic() {
    let listener = TcpConnectionListener::bind("127.0.0.1:0", MAX_FRAME_BYTES)
        .await
        .expect("bind tcp listener");
    let address = listener.local_addr().expect("listener address").to_string();

    let accept = tokio::spawn(async move { listener.accept().await });

    let connector = TcpConnector::new(address).with_max_frame_bytes(MAX_FRAME_BYTES);
    let client = Client::connect(connector)
        .await
        .expect("client connects")
        .with_service(EchoService { side: "client" });
    let server_transport = accept
        .await
        .expect("accept join")
        .expect("accept server transport");

    let server = Server::new(EchoService { side: "server" }, server_transport)
        .with_max_in_flight(CALLS_PER_DIRECTION as usize * 2);

    timeout(TEST_TIMEOUT, run_bidirectional_load(server, client))
        .await
        .expect("tcp bidirectional load finishes within timeout");
}
