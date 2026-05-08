//! c16 acceptance: closing the underlying transport resolves all pending
//! `peer.call` futures with `FittingsError::Transport` and resolves
//! `peer.closed()` on both server and client sides.

use fittings::{
    async_trait::async_trait,
    core::message::{JsonRpcId, Request, Response},
    core::transport::Connector,
    Client, FittingsError, Server, Service, ServiceContext,
};
use fittings_testkit::memory_transport::MemoryTransport;
use serde_json::json;
use std::sync::Arc;
use tokio::{
    sync::Mutex,
    time::{timeout, Duration},
};

struct InertService;

#[async_trait]
impl Service for InertService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        Ok(Response {
            id: req.id.unwrap_or(JsonRpcId::Null),
            result: json!({ "ok": true }),
            metadata: Default::default(),
        })
    }
}

struct OneShotConnector {
    transport: Arc<Mutex<Option<MemoryTransport>>>,
}

impl OneShotConnector {
    fn new(transport: MemoryTransport) -> Self {
        Self {
            transport: Arc::new(Mutex::new(Some(transport))),
        }
    }
}

#[async_trait]
impl Connector for OneShotConnector {
    type Connection = MemoryTransport;

    async fn connect(&self) -> Result<Self::Connection, FittingsError> {
        self.transport
            .lock()
            .await
            .take()
            .ok_or_else(|| FittingsError::internal("connector already used"))
    }
}

#[tokio::test]
async fn server_peer_call_drains_to_transport_error_on_close() {
    let (raw_client, server_transport) = MemoryTransport::pair(16);
    let server = Server::new(InertService, server_transport);
    let peer = server.peer();
    let serve = tokio::spawn(server.serve());

    // Start a peer.call that will never receive a response — the raw client
    // is dropped before sending any reply. Spawn it so we can drop the
    // transport while the call is in flight.
    let call_peer = peer.clone();
    let call = tokio::spawn(async move { call_peer.call("ping", json!({})).await });

    // Give the server a moment to write the request frame, then drop the
    // raw client so the server's transport reads end-of-input.
    tokio::time::sleep(Duration::from_millis(20)).await;
    drop(raw_client);

    let call_result = timeout(Duration::from_millis(500), call)
        .await
        .expect("call future should resolve after transport close")
        .expect("call task join");
    assert!(
        matches!(call_result, Err(FittingsError::Transport(_))),
        "pending peer.call should resolve with Transport error, got {call_result:?}",
    );

    timeout(Duration::from_millis(500), peer.closed())
        .await
        .expect("server peer.closed() should resolve after transport tear-down");

    let _ = timeout(Duration::from_millis(500), serve).await;
}

#[tokio::test]
async fn client_peer_call_drains_to_transport_error_on_close() {
    let (client_transport, server_transport) = MemoryTransport::pair(16);
    let client = Client::connect(OneShotConnector::new(client_transport))
        .await
        .expect("client should connect");
    let peer = client.peer();

    let call_peer = peer.clone();
    let call = tokio::spawn(async move { call_peer.call("ping", json!({})).await });

    tokio::time::sleep(Duration::from_millis(20)).await;
    drop(server_transport);

    let call_result = timeout(Duration::from_millis(500), call)
        .await
        .expect("call future should resolve after transport close")
        .expect("call task join");
    assert!(
        matches!(call_result, Err(FittingsError::Transport(_))),
        "pending client peer.call should resolve with Transport error, got {call_result:?}",
    );

    timeout(Duration::from_millis(500), peer.closed())
        .await
        .expect("client peer.closed() should resolve after transport tear-down");
}
