use std::time::Duration;

use fittings::{
    async_trait::async_trait,
    core::message::{JsonRpcId, Request, Response},
    FittingsError, ResponseEnvelope, Server, Service, ServiceContext, Transport,
};
use fittings_testkit::{fixtures::parse_response_fixture, memory_transport::MemoryTransport};
use serde_json::{json, Value};
use tokio::time::timeout;

/// Sleeps up to `delay_ms` or until the per-request token fires. Returns
/// `Err(Cancelled)` when the token fires; otherwise `Ok`. The handler is
/// responsive to cancellation so each batch item visibly distinguishes
/// "completed" from "suppressed by token".
struct CancellableDelayService;

#[async_trait]
impl Service for CancellableDelayService {
    async fn call(&self, req: Request, ctx: ServiceContext) -> Result<Response, FittingsError> {
        let delay_ms = req
            .params
            .get("delay_ms")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let id = req.id.clone().unwrap_or(JsonRpcId::Null);
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(delay_ms)) => {}
            _ = ctx.cancelled() => {
                return Err(FittingsError::cancelled(None));
            }
        }
        Ok(Response {
            id,
            result: json!({"ok": true}),
            metadata: Default::default(),
        })
    }
}

fn batch_request_line(items: Vec<Value>) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&Value::Array(items)).expect("batch request serialize");
    bytes.push(b'\n');
    bytes
}

fn call_item(id: &str, delay_ms: u64) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "work",
        "params": {"delay_ms": delay_ms},
    })
}

fn cancel_notification(id: &str) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "method": "$/cancelRequest",
        "params": { "id": id },
    }))
    .expect("cancel notification serialize");
    bytes.push(b'\n');
    bytes
}

fn parse_batch_response(frame: &[u8]) -> Vec<ResponseEnvelope> {
    let value: Value = serde_json::from_slice(frame).expect("batch response is valid JSON");
    let items = value
        .as_array()
        .expect("batch response should be a JSON array");
    items
        .iter()
        .map(|item| {
            let item_line = serde_json::to_vec(item).expect("batch item serialize");
            parse_response_fixture(&item_line).expect("batch item decode")
        })
        .collect()
}

#[tokio::test]
async fn cancelling_one_batch_component_suppresses_only_that_response() {
    let (mut client, server_transport) = MemoryTransport::pair(16);
    let server = Server::new(CancellableDelayService, server_transport);
    let handle = tokio::spawn(server.serve());

    let batch = batch_request_line(vec![
        call_item("b-1", 120),
        call_item("b-2", 120),
        call_item("b-3", 120),
    ]);
    client.send(&batch).await.expect("send batch");

    // Let the batch worker pre-register the per-item tokens (RFC rule 2)
    // before the cancellation arrives. Item b-1 has begun executing; b-2
    // and b-3 are still queued but their tokens are already registered,
    // so the cancellation can reach b-2's token before it starts.
    tokio::time::sleep(Duration::from_millis(20)).await;

    client
        .send(&cancel_notification("b-2"))
        .await
        .expect("send cancellation");

    let frame = timeout(Duration::from_millis(800), client.recv())
        .await
        .expect("batch response should arrive")
        .expect("batch response frame");
    let mut responses = parse_batch_response(&frame);
    responses.sort_by(|a, b| a.id.to_string().cmp(&b.id.to_string()));

    assert_eq!(
        responses.len(),
        2,
        "only the two non-cancelled components should produce responses, got: {responses:?}",
    );
    assert_eq!(responses[0].id, JsonRpcId::from("b-1"));
    assert_eq!(responses[1].id, JsonRpcId::from("b-3"));
    assert!(responses.iter().all(|r| r.error.is_none()));

    let stray = timeout(Duration::from_millis(50), client.recv()).await;
    assert!(
        stray.is_err(),
        "no further frames after the single batch response, got: {stray:?}",
    );

    drop(client);
    let result = handle.await.expect("server task join");
    assert!(result.is_ok(), "server should exit cleanly: {result:?}");
}

#[tokio::test]
async fn batch_with_every_component_cancelled_emits_no_response() {
    let (mut client, server_transport) = MemoryTransport::pair(16);
    let server = Server::new(CancellableDelayService, server_transport);
    let handle = tokio::spawn(server.serve());

    let batch = batch_request_line(vec![call_item("all-1", 200), call_item("all-2", 200)]);
    client.send(&batch).await.expect("send batch");

    // Wait for pre-registration of both ids; without it, the second
    // cancellation would race the batch worker and might land before
    // all-2's token exists.
    tokio::time::sleep(Duration::from_millis(20)).await;

    client
        .send(&cancel_notification("all-1"))
        .await
        .expect("send cancellation 1");
    client
        .send(&cancel_notification("all-2"))
        .await
        .expect("send cancellation 2");

    let recv = timeout(Duration::from_millis(600), client.recv()).await;
    assert!(
        recv.is_err(),
        "fully-cancelled batch must not emit any response frame, got: {recv:?}",
    );

    drop(client);
    let result = handle.await.expect("server task join");
    assert!(result.is_ok(), "server should exit cleanly: {result:?}");
}
