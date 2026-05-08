use std::time::Duration;

use fittings::{
    async_trait::async_trait,
    core::message::{Request, Response},
    FittingsError, Server, Service, ServiceContext, Transport,
};
use fittings_testkit::{fixtures::request_line, memory_transport::MemoryTransport};
use serde_json::json;
use tokio::time::timeout;

struct SelfCancelService;

#[async_trait]
impl Service for SelfCancelService {
    async fn call(&self, _req: Request, ctx: ServiceContext) -> Result<Response, FittingsError> {
        assert!(
            !ctx.is_cancelled(),
            "token must not be fired in this scenario"
        );
        Err(FittingsError::cancelled(Some(
            "deadline exceeded".to_string(),
        )))
    }
}

#[tokio::test]
async fn handler_returned_cancelled_suppresses_response_without_token() {
    let (mut client, server_transport) = MemoryTransport::pair(8);
    let server = Server::new(SelfCancelService, server_transport);
    let handle = tokio::spawn(server.serve());

    client
        .send(&request_line("self-cancel", "work", json!({})))
        .await
        .expect("send request");

    let recv = timeout(Duration::from_millis(200), client.recv()).await;
    assert!(
        recv.is_err(),
        "no response frame must be emitted when handler returns Err(Cancelled), got: {recv:?}",
    );

    drop(client);
    let result = handle.await.expect("server task join");
    assert!(result.is_ok(), "server should exit cleanly: {result:?}");
}
