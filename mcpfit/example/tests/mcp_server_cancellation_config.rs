use fittings::{
    async_trait::async_trait,
    core::message::{Request, Response},
    FittingsError, Server, Service, ServiceContext,
};
use fittings_testkit::memory_transport::MemoryTransport;

struct StubService;

#[async_trait]
impl Service for StubService {
    async fn call(&self, _req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        Err(FittingsError::internal("stub"))
    }
}

#[test]
fn mcp_server_configures_mcp_style_cancellation_method_and_extractor() {
    let (_client_transport, server_transport) = MemoryTransport::pair(1);
    let server = mcp_server::configure_cancellation(Server::new(StubService, server_transport));

    assert_eq!(server.cancellation_method(), "notifications/cancelled");
    assert_eq!(server.cancellation_id_field(), "requestId");
}

#[test]
fn mcp_server_cancellation_constants_match_mcp_shape() {
    assert_eq!(mcp_server::CANCELLATION_METHOD, "notifications/cancelled");
    assert_eq!(mcp_server::CANCELLATION_ID_FIELD, "requestId");
}
