use std::time::Duration;

use fittings::{
    async_trait::async_trait,
    core::message::{JsonRpcId, Request, Response},
    Client, Connector, FittingsError, Server, Service, ServiceContext, Transport,
};
use fittings_testkit::{
    fixtures::{parse_response_fixture, request_line},
    memory_transport::MemoryTransport,
};
use serde_json::{json, Value};
use tokio::time::timeout;

const FLOOD_COUNT: usize = 4096;
const NOTIFY_CAPACITY: usize = 8;

struct BurstService;

#[async_trait]
impl Service for BurstService {
    async fn call(&self, req: Request, ctx: ServiceContext) -> Result<Response, FittingsError> {
        match req.method.as_str() {
            "flood" => {
                for index in 0..FLOOD_COUNT {
                    ctx.notify("burst", json!({ "i": index }))
                        .expect("notify never returns Err on drop-on-full path");
                }
                Ok(Response {
                    id: req.id.unwrap_or(JsonRpcId::Null),
                    result: json!({ "emitted": FLOOD_COUNT }),
                    metadata: Default::default(),
                })
            }
            "ping" => Ok(Response {
                id: req.id.unwrap_or(JsonRpcId::Null),
                result: json!({ "pong": true }),
                metadata: Default::default(),
            }),
            other => Err(FittingsError::method_not_found(other.to_string())),
        }
    }
}

async fn drain_until_response_value(
    client: &mut MemoryTransport,
    expected_id: &str,
) -> (usize, Value) {
    let mut notifications = 0usize;
    loop {
        let frame = timeout(Duration::from_secs(5), client.recv())
            .await
            .expect("recv should not stall")
            .expect("recv frame");
        let value: Value = serde_json::from_slice(&frame).expect("valid JSON frame");

        if value.get("id").is_some() {
            let parsed = parse_response_fixture(&frame).expect("parse response");
            assert_eq!(parsed.id, expected_id);
            return (notifications, value);
        }

        assert_eq!(value["method"], "burst");
        notifications += 1;
    }
}

#[tokio::test]
async fn bounded_notify_drops_then_responses_and_subsequent_traffic_succeed() {
    use fittings_testkit::memory_transport::MemoryTransport as MT;

    // Small client buffer + small notify capacity so the bounded sink fills
    // while the dispatcher is still flushing earlier frames.
    let (mut client, server_transport) = MT::pair(2);
    let server =
        Server::new(BurstService, server_transport).with_notification_capacity(NOTIFY_CAPACITY);
    let dropped = server.dropped_notifications();
    let handle = tokio::spawn(server.serve());

    client
        .send(&request_line("flood-1", "flood", json!({})))
        .await
        .expect("send flood request");

    let (notifications, response) = drain_until_response_value(&mut client, "flood-1").await;
    assert!(
        response.get("error").is_none(),
        "flood request should succeed: {response}",
    );

    let dropped_count = dropped.count();
    assert!(
        dropped_count > 0,
        "expected at least one notification to be dropped under flood; got {dropped_count}",
    );
    assert_eq!(
        notifications + dropped_count as usize,
        FLOOD_COUNT,
        "delivered + dropped must equal emitted",
    );
    assert!(
        notifications < FLOOD_COUNT,
        "flood must produce at least one drop",
    );

    // Response channel is independent of the notification sink: a normal
    // request after the flood still completes.
    client
        .send(&request_line("after", "ping", json!({})))
        .await
        .expect("send follow-up request");

    let (post_flood_notifications, after) = drain_until_response_value(&mut client, "after").await;
    assert_eq!(post_flood_notifications, 0);
    assert!(
        after.get("error").is_none(),
        "follow-up request should succeed: {after}",
    );
    assert_eq!(after["result"], json!({ "pong": true }));

    drop(client);
    let result = handle.await.expect("server task join");
    assert!(result.is_ok(), "server should exit cleanly: {result:?}");
}

struct PeerEcho;

#[async_trait]
impl Service for PeerEcho {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        Ok(Response {
            id: req.id.unwrap_or(JsonRpcId::Null),
            result: json!({ "echoed": req.params }),
            metadata: Default::default(),
        })
    }
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

#[tokio::test]
async fn server_peer_call_succeeds_after_bounded_notification_drops() {
    use fittings_testkit::memory_transport::MemoryTransport as MT;

    let (client_transport, server_transport) = MT::pair(2);
    let server =
        Server::new(BurstService, server_transport).with_notification_capacity(NOTIFY_CAPACITY);
    let dropped = server.dropped_notifications();
    let server_peer = server.peer();
    let serve = tokio::spawn(server.serve());

    let client = Client::connect(OneShotConnector {
        transport: tokio::sync::Mutex::new(Some(client_transport)),
    })
    .await
    .expect("client connects")
    .with_service(PeerEcho);

    let flood = client.call("flood", json!({}));
    let result = timeout(Duration::from_secs(5), flood)
        .await
        .expect("flood resolves")
        .expect("flood ok");
    assert_eq!(result, json!({ "emitted": FLOOD_COUNT }));

    let dropped_count = dropped.count();
    assert!(
        dropped_count > 0,
        "expected at least one notification to be dropped under flood; got {dropped_count}",
    );

    // Scope-required post-flood assertion: a peer.call from the server side
    // (using the same connection that just dropped notifications) still
    // completes, because the response/request paths are not blocked by the
    // bounded notification sink.
    let after = timeout(
        Duration::from_secs(5),
        server_peer.call("ping", json!({ "n": 7 })),
    )
    .await
    .expect("post-flood peer.call resolves")
    .expect("post-flood peer.call ok");
    assert_eq!(after, json!({ "echoed": { "n": 7 } }));

    drop(client);
    let result = timeout(Duration::from_secs(5), serve)
        .await
        .expect("serve exits")
        .expect("server task join");
    assert!(result.is_ok(), "server should exit cleanly: {result:?}");
}
