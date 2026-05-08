use std::time::Duration;

use fittings::{
    async_trait::async_trait,
    core::message::{Request, Response},
    FittingsError, Server, Service, ServiceContext, Transport,
};
use fittings_testkit::{fixtures::request_line, memory_transport::MemoryTransport};
use serde_json::json;
use tokio::time::timeout;

struct WaitForCancelService;

#[async_trait]
impl Service for WaitForCancelService {
    async fn call(&self, _req: Request, ctx: ServiceContext) -> Result<Response, FittingsError> {
        let observed = timeout(Duration::from_secs(5), ctx.cancelled()).await;
        assert!(
            observed.is_ok(),
            "handler did not observe cancellation token firing"
        );
        assert!(ctx.is_cancelled());
        Err(FittingsError::cancelled(Some("client aborted".to_string())))
    }
}

fn cancel_notification(id: &str) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "method": "$/cancelRequest",
        "params": { "id": id },
    }))
    .expect("cancel notification should serialize");
    bytes.push(b'\n');
    bytes
}

#[tokio::test]
async fn token_fired_suppresses_response_when_handler_returns() {
    let (mut client, server_transport) = MemoryTransport::pair(8);
    let server = Server::new(WaitForCancelService, server_transport);
    let handle = tokio::spawn(server.serve());

    client
        .send(&request_line("hold", "work", json!({})))
        .await
        .expect("send saturating request");

    tokio::time::sleep(Duration::from_millis(50)).await;

    client
        .send(&cancel_notification("hold"))
        .await
        .expect("send cancellation notification");

    let recv = timeout(Duration::from_millis(200), client.recv()).await;
    assert!(
        recv.is_err(),
        "no response frame must be emitted when token fired and handler returned Cancelled, got: {recv:?}",
    );

    drop(client);
    let result = handle.await.expect("server task join");
    assert!(result.is_ok(), "server should exit cleanly: {result:?}");
}
