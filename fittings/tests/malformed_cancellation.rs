use std::time::Duration;

use fittings::{
    async_trait::async_trait,
    core::message::{Request, Response},
    wire::types::JsonRpcId,
    FittingsError, Server, Service, ServiceContext, Transport,
};
use fittings_testkit::{
    fixtures::{parse_response_fixture, request_line},
    memory_transport::MemoryTransport,
};
use serde_json::{json, Value};
use tokio::time::timeout;

/// Sleeps long enough that the dispatcher is forced to process subsequent
/// frames while this request is still in-flight. Returns a normal success
/// response — malformed cancellation frames must not affect this request.
struct SlowOkService;

#[async_trait]
impl Service for SlowOkService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        let delay_ms = req
            .params
            .get("delay_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or(50);
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        Ok(Response {
            id: req.id.unwrap_or(JsonRpcId::Null),
            result: json!({"ok": true}),
            metadata: Default::default(),
        })
    }
}

#[derive(Clone, Copy)]
enum ExtractorConfig {
    LspDefault,
    McpOverride,
}

impl ExtractorConfig {
    fn method(self) -> &'static str {
        match self {
            ExtractorConfig::LspDefault => "$/cancelRequest",
            ExtractorConfig::McpOverride => "notifications/cancelled",
        }
    }

    fn id_field(self) -> &'static str {
        match self {
            ExtractorConfig::LspDefault => "id",
            ExtractorConfig::McpOverride => "requestId",
        }
    }

    fn apply(
        self,
        server: Server<SlowOkService, MemoryTransport>,
    ) -> Server<SlowOkService, MemoryTransport> {
        match self {
            ExtractorConfig::LspDefault => server,
            ExtractorConfig::McpOverride => {
                server.with_cancellation(self.method(), self.id_field())
            }
        }
    }
}

#[derive(Clone, Copy)]
enum MalformedShape {
    NonObjectParams,
    MissingIdField,
    IdTypeMismatch,
}

#[derive(Clone, Copy)]
enum InFlightId {
    Str(&'static str),
    Num(i64),
}

impl InFlightId {
    fn to_jsonrpc_id(self) -> JsonRpcId {
        match self {
            InFlightId::Str(s) => JsonRpcId::from(s),
            InFlightId::Num(n) => JsonRpcId::from(n),
        }
    }
}

fn cancel_frame(extractor: ExtractorConfig, params: Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "method": extractor.method(),
        "params": params,
    }))
    .expect("cancellation frame should serialize");
    bytes.push(b'\n');
    bytes
}

fn cancel_frame_no_params(extractor: ExtractorConfig) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "method": extractor.method(),
        "params": [1, 2, 3],
    }))
    .expect("cancellation frame should serialize");
    bytes.push(b'\n');
    bytes
}

fn build_malformed(
    extractor: ExtractorConfig,
    shape: MalformedShape,
    in_flight: InFlightId,
) -> Vec<u8> {
    match shape {
        MalformedShape::NonObjectParams => cancel_frame_no_params(extractor),
        MalformedShape::MissingIdField => cancel_frame(extractor, json!({"reason": "stale"})),
        MalformedShape::IdTypeMismatch => {
            // Build a cancel id whose JSON `to_string()` matches the
            // in-flight key's `to_string()` but uses a different JSON
            // kind. This is the genuine same-logical-id type-mismatch
            // path scope §S7.1 calls out: "string id sent for a
            // numeric in-flight key or vice versa".
            let cancel_id = match in_flight {
                InFlightId::Str(s) => json!(s.parse::<i64>().expect("numeric-shaped string id")),
                InFlightId::Num(n) => json!(n.to_string()),
            };
            cancel_frame(extractor, json!({ extractor.id_field(): cancel_id }))
        }
    }
}

async fn run_case(extractor: ExtractorConfig, shape: MalformedShape, in_flight: InFlightId) {
    let (mut client, server_transport) = MemoryTransport::pair(16);
    let server = extractor.apply(Server::new(SlowOkService, server_transport));
    let handle = tokio::spawn(server.serve());

    let in_flight_id = in_flight.to_jsonrpc_id();

    // Saturate the dispatcher with a real in-flight request so the
    // malformed cancellation must not kill it.
    client
        .send(&request_line(
            in_flight_id.clone(),
            "work",
            json!({"delay_ms": 80}),
        ))
        .await
        .expect("send in-flight request");

    // Give the handler a moment to start.
    tokio::time::sleep(Duration::from_millis(20)).await;

    client
        .send(&build_malformed(extractor, shape, in_flight))
        .await
        .expect("send malformed cancellation");

    // The malformed cancellation must not affect other requests. Send a
    // second request and require it to receive a normal response. The
    // follow-up id is unconditionally a string so it never collides with
    // the numeric in-flight cases.
    client
        .send(&request_line("ping-1", "work", json!({"delay_ms": 0})))
        .await
        .expect("send follow-up request");

    let mut received = Vec::new();
    while received.len() < 2 {
        let frame = timeout(Duration::from_millis(500), client.recv())
            .await
            .expect("server should respond within deadline")
            .expect("recv frame");
        let response = parse_response_fixture(&frame).expect("parse response");
        received.push(response);
    }

    let ping_id = JsonRpcId::from("ping-1");
    let in_flight_pos = received
        .iter()
        .position(|r| r.id == in_flight_id)
        .expect("in-flight response present");
    let ping_pos = received
        .iter()
        .position(|r| r.id == ping_id)
        .expect("follow-up response present");
    assert_ne!(in_flight_pos, ping_pos);
    assert!(
        received.iter().all(|r| r.error.is_none()),
        "in-flight requests should succeed despite malformed cancellation: {received:?}"
    );

    drop(client);
    let result = handle.await.expect("server task join");
    assert!(
        result.is_ok(),
        "malformed cancellation must not kill the connection: {result:?}",
    );
}

#[tokio::test]
async fn malformed_cancellation_is_logged_and_dropped() {
    let configs = [ExtractorConfig::LspDefault, ExtractorConfig::McpOverride];

    // Shape × in-flight-id matrix. The non-mismatch shapes don't depend
    // on the in-flight kind, so they're exercised once with a string id;
    // the type-mismatch shape is exercised with both directions
    // (string-vs-numeric and numeric-vs-string) per scope §S7.1.
    let cases: &[(MalformedShape, InFlightId)] = &[
        (MalformedShape::NonObjectParams, InFlightId::Str("hold-1")),
        (MalformedShape::MissingIdField, InFlightId::Str("hold-1")),
        (MalformedShape::IdTypeMismatch, InFlightId::Str("42")),
        (MalformedShape::IdTypeMismatch, InFlightId::Num(42)),
    ];

    for extractor in configs {
        for (shape, in_flight) in cases.iter().copied() {
            run_case(extractor, shape, in_flight).await;
        }
    }
}
