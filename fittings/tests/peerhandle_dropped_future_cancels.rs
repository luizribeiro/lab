//! c18 acceptance: when a `peer.call` future is dropped before it resolves,
//! the server-side `PeerHandle` emits the configured cancellation
//! notification on the wire (LSP default `$/cancelRequest`; MCP override
//! `notifications/cancelled`) naming the dropped call's id, and the
//! `pending_outbound` slot for that id is vacated so a subsequent call can
//! reuse the channel.

use fittings::{
    async_trait::async_trait,
    core::message::{JsonRpcId, Request, Response},
    FittingsError, Server, Service, ServiceContext, Transport,
};
use fittings_testkit::memory_transport::MemoryTransport;
use serde_json::{json, Value};
use tokio::time::{timeout, Duration};

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

fn parse_frame(frame: &[u8]) -> Value {
    serde_json::from_slice(frame).expect("frame should be valid JSON")
}

/// Pull frames off the raw client until either a peer.call request frame
/// (carrying an `s_<n>` id) or a notification frame is observed. Returns
/// the parsed JSON value.
async fn recv_frame(client: &mut MemoryTransport) -> Value {
    let frame = timeout(Duration::from_millis(500), client.recv())
        .await
        .expect("frame should arrive before timeout")
        .expect("transport recv should succeed");
    parse_frame(&frame)
}

async fn run_dropped_cancel_scenario(
    cancellation_method: Option<(&'static str, &'static str)>,
    expected_method: &str,
    expected_id_field: &str,
) {
    let (mut raw_client, server_transport) = MemoryTransport::pair(16);
    let mut server = Server::new(InertService, server_transport);
    if let Some((method, id_field)) = cancellation_method {
        server = server.with_cancellation(method, id_field);
    }

    let peer = server.peer();
    let serve = tokio::spawn(server.serve());

    // Spawn the peer.call so we can drop it from outside without resolving.
    let call_peer = peer.clone();
    let call_handle =
        tokio::spawn(async move { call_peer.call("slow", json!({ "wait": true })).await });

    // The server forwards the request frame to the wire — observe it and
    // capture the id the allocator picked.
    let request = recv_frame(&mut raw_client).await;
    let request_id = request
        .get("id")
        .cloned()
        .expect("server-initiated request should carry an id");
    assert!(
        request_id.as_str().is_some_and(|s| s.starts_with("s_")),
        "server peer.call must use the s_ namespace, got {request_id:?}",
    );
    assert_eq!(request.get("method").and_then(Value::as_str), Some("slow"));

    // Drop the future *without* sending a response. The PeerHandle should
    // emit a cancellation notification on the wire and clear its pending
    // slot.
    call_handle.abort();
    let _ = call_handle.await;

    let notification = recv_frame(&mut raw_client).await;
    assert!(
        notification.get("id").is_none(),
        "cancellation must be a notification (no id), got {notification}",
    );
    assert_eq!(
        notification.get("method").and_then(Value::as_str),
        Some(expected_method),
        "cancellation method should match configuration",
    );
    let params = notification
        .get("params")
        .cloned()
        .expect("cancellation should carry params");
    assert_eq!(
        params.get(expected_id_field),
        Some(&request_id),
        "cancellation params must name the dropped call's id under the configured field",
    );

    // The pending slot must now be vacated: a fresh peer.call can complete
    // through the same connection. The raw client echoes the next request.
    let next_call = tokio::spawn({
        let peer = peer.clone();
        async move { peer.call("ping", json!({ "n": 1 })).await }
    });

    let next_request = recv_frame(&mut raw_client).await;
    let next_id = next_request
        .get("id")
        .cloned()
        .expect("follow-up request should carry an id");
    assert_ne!(
        next_id, request_id,
        "follow-up call should use a fresh id, distinct from the cancelled one",
    );
    let id_str = next_id
        .as_str()
        .expect("server-initiated id should be a string")
        .to_owned();
    let response_bytes = {
        let mut bytes = serde_json::to_vec(&json!({
            "jsonrpc": "2.0",
            "id": id_str,
            "result": { "echoed": "pong" },
        }))
        .expect("encode response");
        bytes.push(b'\n');
        bytes
    };
    raw_client
        .send(&response_bytes)
        .await
        .expect("send response");

    let result = timeout(Duration::from_millis(500), next_call)
        .await
        .expect("follow-up peer.call should resolve")
        .expect("follow-up peer.call task join")
        .expect("follow-up peer.call should succeed");
    assert_eq!(result, json!({ "echoed": "pong" }));

    drop(peer);
    drop(raw_client);
    let serve_result = timeout(Duration::from_millis(200), serve)
        .await
        .expect("serve should exit promptly")
        .expect("server task join");
    assert!(
        serve_result.is_ok(),
        "server should exit cleanly: {serve_result:?}",
    );
}

#[tokio::test]
async fn dropped_peer_call_emits_lsp_cancellation_notification() {
    run_dropped_cancel_scenario(None, "$/cancelRequest", "id").await;
}

#[tokio::test]
async fn dropped_peer_call_emits_mcp_cancellation_notification() {
    run_dropped_cancel_scenario(
        Some(("notifications/cancelled", "requestId")),
        "notifications/cancelled",
        "requestId",
    )
    .await;
}
