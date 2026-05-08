//! c15 acceptance: a handler can issue `ctx.peer().call(...)` (and
//! `ctx.peer().notify(...)`) to the peer while its own request is in flight,
//! await the peer response, and then return its own response. Tests both
//! directions on one connection (client → server, server → client).

use fittings::{
    async_trait::async_trait,
    core::message::{JsonRpcId, Request, Response},
    Client, Connector, FittingsError, Server, Service, ServiceContext,
};
use fittings_testkit::memory_transport::MemoryTransport;
use serde_json::json;
use tokio::time::{timeout, Duration};

/// A service whose `outer` handler reaches back through `ctx.peer()` to call
/// the peer's `inner` method mid-flight. Its own response wraps the peer
/// response, proving the inner call resolved before the outer handler
/// returned. Also emits a `progress` notification via `ctx.peer().notify` so
/// the test exercises both peer-side primitives from inside a handler.
struct WrappingService {
    side: &'static str,
}

#[async_trait]
impl Service for WrappingService {
    async fn call(&self, req: Request, ctx: ServiceContext) -> Result<Response, FittingsError> {
        let id = req.id.clone().unwrap_or(JsonRpcId::Null);
        match req.method.as_str() {
            "outer" => {
                ctx.peer()
                    .notify(
                        "progress",
                        json!({ "from": self.side, "phase": "before-inner" }),
                    )
                    .expect("peer notify should enqueue");

                let inner = ctx
                    .peer()
                    .call("inner", json!({ "from": self.side }))
                    .await
                    .expect("inner peer.call should succeed");

                Ok(Response {
                    id,
                    result: json!({ "wrapped_by": self.side, "inner": inner }),
                    metadata: Default::default(),
                })
            }
            "inner" => Ok(Response {
                id,
                result: json!({ "answered_by": self.side, "echo": req.params }),
                metadata: Default::default(),
            }),
            other => Err(FittingsError::method_not_found(other)),
        }
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
async fn handler_calls_peer_mid_flight_in_both_directions() {
    let (client_transport, server_transport) = MemoryTransport::pair(64);

    let server = Server::new(WrappingService { side: "server" }, server_transport);
    let server_peer = server.peer();
    let serve = tokio::spawn(server.serve());

    let client = Client::connect(OneShotConnector {
        transport: tokio::sync::Mutex::new(Some(client_transport)),
    })
    .await
    .expect("client connects")
    .with_service(WrappingService { side: "client" });

    // Direction 1: client → server.outer → ctx.peer().call("inner") → client.
    let c2s = client
        .call("outer", json!({}))
        .await
        .expect("client→server outer call");
    assert_eq!(
        c2s,
        json!({
            "wrapped_by": "server",
            "inner": { "answered_by": "client", "echo": { "from": "server" } },
        }),
        "server's outer handler should wrap the client's inner response",
    );

    // Direction 2: server → client.outer → ctx.peer().call("inner") → server.
    let s2c = timeout(
        Duration::from_millis(500),
        server_peer.call("outer", json!({})),
    )
    .await
    .expect("server→client outer call resolves")
    .expect("server peer.call ok");
    assert_eq!(
        s2c,
        json!({
            "wrapped_by": "client",
            "inner": { "answered_by": "server", "echo": { "from": "client" } },
        }),
        "client's outer handler should wrap the server's inner response",
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
