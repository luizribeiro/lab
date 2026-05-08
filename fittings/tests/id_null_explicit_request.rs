//! c20 acceptance: an inbound `"id": null` envelope is a request (not a
//! notification). The handler runs and the response carries `"id": null`,
//! per `rfc-fittings-notifications.md:137-145`.

use fittings::{
    async_trait::async_trait,
    core::message::{JsonRpcId, Request, Response},
    FittingsError, Server, Service, ServiceContext, Transport,
};
use fittings_testkit::{fixtures::parse_response_fixture, memory_transport::MemoryTransport};
use serde_json::json;
use tokio::time::{timeout, Duration};

struct EchoService;

#[async_trait]
impl Service for EchoService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        Ok(Response {
            id: req.id.unwrap_or(JsonRpcId::Null),
            result: json!({"method": req.method}),
            metadata: Default::default(),
        })
    }
}

#[tokio::test]
async fn explicit_null_id_request_gets_null_id_response() {
    let (mut client, server_transport) = MemoryTransport::pair(8);
    let server = Server::new(EchoService, server_transport);
    let server_handle = tokio::spawn(server.serve());

    client
        .send(b"{\"jsonrpc\":\"2.0\",\"id\":null,\"method\":\"ping\",\"params\":{}}\n")
        .await
        .expect("send explicit-null-id request");

    let frame = timeout(Duration::from_millis(500), client.recv())
        .await
        .expect("client should receive a response within the timeout")
        .expect("client recv");
    let response = parse_response_fixture(&frame).expect("parse response");

    assert_eq!(
        response.id,
        JsonRpcId::Null,
        "explicit null-id request must produce a response carrying `id: null`",
    );
    assert!(
        response.error.is_none(),
        "handler should have run and produced a successful result"
    );
    assert_eq!(response.result, Some(json!({"method": "ping"})));

    drop(client);
    let _ = server_handle.await.expect("server task should join");
}

#[tokio::test]
async fn missing_id_notification_does_not_produce_a_response_alongside_null_id_request() {
    let (mut client, server_transport) = MemoryTransport::pair(8);
    let server = Server::new(EchoService, server_transport);
    let server_handle = tokio::spawn(server.serve());

    client
        .send(b"{\"jsonrpc\":\"2.0\",\"method\":\"ping\",\"params\":{}}\n")
        .await
        .expect("send notification (missing id)");
    client
        .send(b"{\"jsonrpc\":\"2.0\",\"id\":null,\"method\":\"ping\",\"params\":{}}\n")
        .await
        .expect("send explicit-null-id request");

    let frame = timeout(Duration::from_millis(500), client.recv())
        .await
        .expect("client should receive the null-id response within the timeout")
        .expect("client recv");
    let response = parse_response_fixture(&frame).expect("parse response");
    assert_eq!(response.id, JsonRpcId::Null);
    assert!(response.error.is_none());

    let extra = timeout(Duration::from_millis(50), client.recv()).await;
    assert!(
        extra.is_err(),
        "the notification (missing id) must not produce its own response"
    );

    drop(client);
    let _ = server_handle.await.expect("server task should join");
}
