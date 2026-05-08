//! c10 acceptance: `Server::peer().notify(...)` invoked from a startup task
//! (i.e. outside any inbound handler) is observable on the wire by a raw
//! test-harness client that reads frames directly off the transport rather
//! than going through `fittings::Client`.
//!
//! Group 3 commits extend this file with `peer.call` / `peer.closed()` and the
//! client-side mirror once K2's notification handler registration lands.

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

fn parse_outbound_frame(frame: &[u8]) -> Value {
    serde_json::from_slice(frame).expect("server frame should be valid JSON")
}

#[tokio::test]
async fn server_peer_notify_from_startup_task_reaches_raw_client() {
    let (mut raw_client, server_transport) = MemoryTransport::pair(16);
    let server = Server::new(InertService, server_transport);

    let peer = server.peer();
    let serve = tokio::spawn(server.serve());

    // A startup task — explicitly *outside* any inbound handler — pushes a
    // notification through the connection-scoped peer handle. This is the
    // shape the renderer/peer pattern (decisions.md row 18) relies on.
    let startup = tokio::spawn(async move {
        peer.notify("startup/ready", json!({ "phase": "boot" }))
            .expect("peer.notify should enqueue from outside a handler");
    });
    startup.await.expect("startup task should complete");

    // Raw test harness: read framed JSON directly off the transport rather
    // than going through `fittings::Client` (Client::peer notifications land
    // in c19 once the inbound notification handler exists on the client).
    let frame = timeout(Duration::from_millis(500), raw_client.recv())
        .await
        .expect("notification should arrive before timeout")
        .expect("raw client should receive a frame");

    let value = parse_outbound_frame(&frame);
    assert!(
        value.get("id").is_none(),
        "frame must be a JSON-RPC notification (no id field): {value}",
    );
    assert_eq!(value["jsonrpc"], "2.0");
    assert_eq!(value["method"], "startup/ready");
    assert_eq!(value["params"], json!({ "phase": "boot" }));

    drop(raw_client);
    let result = serve.await.expect("server task join");
    assert!(result.is_ok(), "server should exit cleanly: {result:?}");
}

#[tokio::test]
async fn server_peer_handle_clones_share_drop_counter() {
    // Sanity: the peer surfaced via `Server::peer()` is the same connection
    // -scoped sink as the one available inside handlers — its dropped
    // counter mirrors the server's `dropped_notifications()`. This anchors
    // the c09 invariant against the new accessor.
    let (_raw_client, server_transport) = MemoryTransport::pair(2);
    let server = Server::new(InertService, server_transport).with_notification_capacity(1);

    let peer = server.peer();
    let server_counter = server.dropped_notifications();

    // Fill the bounded channel and force a drop without running serve(),
    // so nothing is being read off the notification side. The first send
    // fits, the rest are dropped on full.
    peer.notify("ev", json!({ "n": 0 }))
        .expect("first notify fits");
    peer.notify("ev", json!({ "n": 1 }))
        .expect("drop-on-full reports Ok");
    peer.notify("ev", json!({ "n": 2 }))
        .expect("drop-on-full reports Ok");

    assert_eq!(server_counter.count(), 2);
    assert_eq!(peer.dropped_notifications().count(), 2);
}
