//! c10 acceptance: `Server::peer().notify(...)` invoked from a startup task
//! (i.e. outside any inbound handler) is observable on the wire by a raw
//! test-harness client that reads frames directly off the transport rather
//! than going through `fittings::Client`.
//!
//! Group 3 commits extend this file with `peer.call` / `peer.closed()` and the
//! client-side mirror once K2's notification handler registration lands.

use std::sync::{Arc, Mutex};

use fittings::{
    async_trait::async_trait,
    core::message::{JsonRpcId, Request, Response},
    Client, Connector, FittingsError, Server, Service, ServiceContext, Transport,
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

/// Server-side service that records every method it sees (including
/// notifications, which arrive as id-less requests routed to the same
/// `Service`).
struct RecordingService {
    seen: Arc<Mutex<Vec<(String, Value)>>>,
}

#[async_trait]
impl Service for RecordingService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        self.seen
            .lock()
            .expect("seen poisoned")
            .push((req.method.clone(), req.params.clone()));
        Ok(Response {
            id: req.id.unwrap_or(JsonRpcId::Null),
            result: json!({ "ack": true }),
            metadata: Default::default(),
        })
    }
}

#[tokio::test]
async fn outside_handler_notifications_round_trip_through_public_apis() {
    // c19 acceptance: with the client-side notification handler now public,
    // both `Server::peer().notify(...)` and `Client::peer().notify(...)` are
    // observable through the public API in their respective peers — no raw
    // wire harness required.
    let (client_transport, server_transport) = MemoryTransport::pair(16);

    let server_seen = Arc::new(Mutex::new(Vec::<(String, Value)>::new()));
    let server = Server::new(
        RecordingService {
            seen: Arc::clone(&server_seen),
        },
        server_transport,
    );
    let server_peer = server.peer();
    let serve = tokio::spawn(server.serve());

    let client_seen = Arc::new(Mutex::new(Vec::<(String, Value)>::new()));
    let client_seen_for_handler = Arc::clone(&client_seen);
    let client = Client::connect(OneShotConnector {
        transport: tokio::sync::Mutex::new(Some(client_transport)),
    })
    .await
    .expect("client connects")
    .with_notification_handler(move |method, params| {
        client_seen_for_handler
            .lock()
            .expect("client_seen poisoned")
            .push((method, params));
    });

    // Startup task on the client side: drive an outbound notification
    // through `Client::peer().notify(...)` — the public-API mirror of S1.
    let client_peer = client.peer();
    tokio::spawn(async move {
        client_peer
            .notify("client/ready", json!({ "phase": "boot" }))
            .expect("client peer.notify enqueues");
    })
    .await
    .expect("client startup task join");

    // Startup task on the server side: drive an outbound notification via
    // `Server::peer().notify(...)`. The client's registered notification
    // handler observes it.
    tokio::spawn(async move {
        server_peer
            .notify("server/ready", json!({ "phase": "boot" }))
            .expect("server peer.notify enqueues");
    })
    .await
    .expect("server startup task join");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    loop {
        let server_done = !server_seen.lock().expect("server_seen").is_empty();
        let client_done = !client_seen.lock().expect("client_seen").is_empty();
        if server_done && client_done {
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            panic!(
                "timed out waiting for both notifications: server={:?}, client={:?}",
                server_seen.lock().expect("server_seen"),
                client_seen.lock().expect("client_seen"),
            );
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let server_observed = server_seen.lock().expect("server_seen").clone();
    assert!(
        server_observed
            .iter()
            .any(|(m, p)| m == "client/ready" && p == &json!({ "phase": "boot" })),
        "server's Service should observe the client's outbound notification: {server_observed:?}",
    );

    let client_observed = client_seen.lock().expect("client_seen").clone();
    assert_eq!(
        client_observed,
        vec![("server/ready".to_string(), json!({ "phase": "boot" }),)],
    );

    drop(client);
    let serve_result = timeout(Duration::from_millis(500), serve)
        .await
        .expect("serve exits")
        .expect("server task join");
    assert!(
        serve_result.is_ok(),
        "server should exit cleanly: {serve_result:?}"
    );
}
