//! c12 acceptance: `Server::peer().call(method, params).await` resolves with
//! the peer's response. A hand-rolled echo client recognises the server's
//! `s_<n>` ids and replies with a result; the server routes the inbound
//! response to its `pending_outbound` map and wakes the call future.

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

async fn run_echo_loop(mut raw_client: MemoryTransport) {
    while let Ok(frame) = raw_client.recv().await {
        let value = parse_frame(&frame);
        let id = match value.get("id") {
            Some(id) => id.clone(),
            None => continue,
        };
        let id_str = match id.as_str() {
            Some(s) if s.starts_with("s_") => s.to_owned(),
            _ => continue,
        };
        let params = value.get("params").cloned().unwrap_or(Value::Null);
        let response = json!({
            "jsonrpc": "2.0",
            "id": id_str,
            "result": { "echoed": params },
        });
        let mut bytes = serde_json::to_vec(&response).expect("serialize response");
        bytes.push(b'\n');
        if raw_client.send(&bytes).await.is_err() {
            return;
        }
    }
}

#[tokio::test]
async fn server_peer_call_resolves_with_echo_clients_response() {
    let (raw_client, server_transport) = MemoryTransport::pair(16);
    let server = Server::new(InertService, server_transport);

    let peer = server.peer();
    let serve = tokio::spawn(server.serve());
    let echo = tokio::spawn(run_echo_loop(raw_client));

    let result = timeout(
        Duration::from_millis(500),
        peer.call("ping", json!({ "n": 1 })),
    )
    .await
    .expect("peer.call should resolve before timeout")
    .expect("peer.call should succeed");

    assert_eq!(result, json!({ "echoed": { "n": 1 } }));

    drop(peer);
    echo.abort();
    let _ = echo.await;
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
async fn server_peer_call_dispatches_concurrent_calls_to_distinct_ids() {
    let (raw_client, server_transport) = MemoryTransport::pair(16);
    let server = Server::new(InertService, server_transport);

    let peer = server.peer();
    let serve = tokio::spawn(server.serve());
    let echo = tokio::spawn(run_echo_loop(raw_client));

    let peer_a = peer.clone();
    let peer_b = peer.clone();
    let call_a = tokio::spawn(async move { peer_a.call("a", json!({ "tag": "a" })).await });
    let call_b = tokio::spawn(async move { peer_b.call("b", json!({ "tag": "b" })).await });

    let a = timeout(Duration::from_millis(500), call_a)
        .await
        .expect("call a resolves")
        .expect("call a join")
        .expect("call a ok");
    let b = timeout(Duration::from_millis(500), call_b)
        .await
        .expect("call b resolves")
        .expect("call b join")
        .expect("call b ok");

    assert_eq!(a, json!({ "echoed": { "tag": "a" } }));
    assert_eq!(b, json!({ "echoed": { "tag": "b" } }));

    drop(peer);
    echo.abort();
    let _ = echo.await;
    let serve_result = timeout(Duration::from_millis(200), serve)
        .await
        .expect("serve should exit promptly")
        .expect("server task join");
    assert!(serve_result.is_ok());
}

#[tokio::test]
async fn server_peer_call_propagates_error_response() {
    let (mut raw_client, server_transport) = MemoryTransport::pair(16);
    let server = Server::new(InertService, server_transport);

    let peer = server.peer();
    let serve = tokio::spawn(server.serve());

    let error_responder = tokio::spawn(async move {
        while let Ok(frame) = raw_client.recv().await {
            let value = parse_frame(&frame);
            let id = match value.get("id").and_then(Value::as_str) {
                Some(s) if s.starts_with("s_") => s.to_owned(),
                _ => continue,
            };
            let response = json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": "not found" },
            });
            let mut bytes = serde_json::to_vec(&response).expect("serialize response");
            bytes.push(b'\n');
            if raw_client.send(&bytes).await.is_err() {
                return;
            }
        }
    });

    let err = timeout(Duration::from_millis(500), peer.call("missing", json!({})))
        .await
        .expect("peer.call should resolve")
        .expect_err("peer.call should fail with the peer's error");
    assert!(
        matches!(
            err,
            FittingsError::MethodNotFound { ref message, .. } if message == "not found"
        ),
        "expected MethodNotFound, got {err:?}",
    );

    drop(peer);
    error_responder.abort();
    let _ = error_responder.await;
    let serve_result = timeout(Duration::from_millis(200), serve)
        .await
        .expect("serve should exit promptly")
        .expect("server task join");
    assert!(serve_result.is_ok());
}
