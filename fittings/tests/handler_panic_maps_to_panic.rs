use fittings::{
    async_trait::async_trait,
    core::message::{Request, Response},
    Client, Connector, FittingsError, Server, Service, ServiceContext,
};
use fittings_testkit::memory_transport::MemoryTransport;
use serde_json::json;

struct PanickingService;

#[async_trait]
impl Service for PanickingService {
    async fn call(&self, _req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        panic!("kaboom from handler");
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
async fn handler_panic_surfaces_as_fittings_error_panic_via_marker() {
    let (client_transport, server_transport) = MemoryTransport::pair(16);
    let server = Server::new(PanickingService, server_transport);
    let server_handle = tokio::spawn(server.serve());

    let client = Client::connect(OneShotConnector {
        transport: tokio::sync::Mutex::new(Some(client_transport)),
    })
    .await
    .expect("client should connect");

    let error = client
        .call("explode", json!({}))
        .await
        .expect_err("call should fail with the marker-decoded panic variant");

    match error {
        FittingsError::Panic { message } => {
            assert_eq!(
                message, "kaboom from handler",
                "panic payload string should round-trip via the fittingsKind marker"
            );
        }
        other => panic!("expected FittingsError::Panic, got {other:?}"),
    }

    drop(client);
    // serve() returns Err once the worker panic surfaces and tears the loop down.
    let _ = server_handle.await.expect("server task should join");
}
