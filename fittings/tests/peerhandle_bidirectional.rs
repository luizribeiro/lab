//! c14 acceptance: server- and client-initiated `peer.call`s coexist on the
//! same connection. The server uses its `Service` (passed to
//! `Server::new(service, _)`) to answer client-initiated calls; the client
//! uses `with_service` to answer server-initiated calls. Both directions
//! complete successfully on one fd.

use fittings::{
    async_trait::async_trait,
    core::message::{JsonRpcId, Request, Response},
    Client, Connector, FittingsError, Server, Service, ServiceContext,
};
use fittings_testkit::memory_transport::MemoryTransport;
use serde_json::json;
use tokio::time::{timeout, Duration};

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

struct OneShotConnector {
    transport: tokio::sync::Mutex<Option<MemoryTransport>>,
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
async fn server_and_client_initiated_peer_calls_succeed_on_one_connection() {
    let (client_transport, server_transport) = MemoryTransport::pair(64);

    let server = Server::new(EchoService { side: "server" }, server_transport);
    let server_peer = server.peer();
    let serve = tokio::spawn(server.serve());

    let client = Client::connect(OneShotConnector {
        transport: tokio::sync::Mutex::new(Some(client_transport)),
    })
    .await
    .expect("client connects")
    .with_service(EchoService { side: "client" });
    let client_peer = client.peer();

    // Both directions concurrently.
    let server_to_client = tokio::spawn(async move {
        server_peer
            .call("server-asks", json!({ "n": 1 }))
            .await
            .expect("server peer.call ok")
    });
    let client_to_server = tokio::spawn(async move {
        client_peer
            .call("client-asks", json!({ "n": 2 }))
            .await
            .expect("client peer.call ok")
    });

    let s_result = timeout(Duration::from_millis(500), server_to_client)
        .await
        .expect("server-initiated call resolves")
        .expect("join");
    let c_result = timeout(Duration::from_millis(500), client_to_server)
        .await
        .expect("client-initiated call resolves")
        .expect("join");

    assert_eq!(
        s_result,
        json!({ "side": "client", "method": "server-asks", "params": { "n": 1 } }),
        "server-initiated call should be answered by the client's registered service",
    );
    assert_eq!(
        c_result,
        json!({ "side": "server", "method": "client-asks", "params": { "n": 2 } }),
        "client-initiated call should be answered by the server's registered service",
    );

    drop(client);
    let serve_result = timeout(Duration::from_millis(500), serve)
        .await
        .expect("serve exits")
        .expect("server task join");
    assert!(
        serve_result.is_ok(),
        "server should exit cleanly: {serve_result:?}"
    );
}
