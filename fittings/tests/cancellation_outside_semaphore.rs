use std::time::Duration;

use fittings::{
    async_trait::async_trait,
    core::message::{JsonRpcId, Request, Response},
    FittingsError, Server, Service, ServiceContext, Transport,
};
use fittings_testkit::{
    fixtures::{parse_response_fixture, request_line},
    memory_transport::MemoryTransport,
};
use serde_json::{json, Value};
use tokio::time::timeout;

/// Sleeps until the per-request cancellation token fires, then returns
/// a normal response. If the token never fires, the handler eventually
/// times out so a hung dispatcher is surfaced as a test failure rather
/// than as an indefinite hang.
struct WaitForCancelService;

#[async_trait]
impl Service for WaitForCancelService {
    async fn call(&self, req: Request, ctx: ServiceContext) -> Result<Response, FittingsError> {
        let observed = timeout(Duration::from_secs(5), ctx.cancelled()).await;
        assert!(
            observed.is_ok(),
            "handler did not observe cancellation token firing"
        );
        Ok(Response {
            id: req.id.unwrap_or(JsonRpcId::Null),
            result: json!({ "cancelled": true }),
            metadata: Default::default(),
        })
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
async fn cancellation_observed_while_handler_saturates_semaphore() {
    let (mut client, server_transport) = MemoryTransport::pair(8);
    let server = Server::new(WaitForCancelService, server_transport).with_max_in_flight(1);
    let handle = tokio::spawn(server.serve());

    client
        .send(&request_line("hold", "work", json!({})))
        .await
        .expect("send saturating request");

    // Give the dispatcher a moment to acquire the only permit and start
    // the handler, so the next inbound frame would block on the
    // semaphore if the cancellation reader were not routed outside it.
    tokio::time::sleep(Duration::from_millis(50)).await;

    client
        .send(&cancel_notification("hold"))
        .await
        .expect("send cancellation notification");

    let frame = timeout(Duration::from_secs(2), client.recv())
        .await
        .expect("response should arrive after cancellation routes outside the semaphore")
        .expect("response frame present");
    let value: Value = serde_json::from_slice(&frame).expect("frame is valid JSON");
    assert!(
        value.get("id").is_some(),
        "expected a response frame, got: {value}"
    );
    let response = parse_response_fixture(&frame).expect("parse response");
    assert_eq!(response.id, JsonRpcId::from("hold"));
    assert!(response.error.is_none(), "response should be success");

    drop(client);
    let result = handle.await.expect("server task join");
    assert!(result.is_ok(), "server should exit cleanly: {result:?}");
}
