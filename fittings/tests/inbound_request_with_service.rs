//! c13 acceptance: a `Client::with_service(svc)` routes peer-originated
//! inbound requests to the registered handler. The handler's response
//! travels back to the peer over the same connection.

use std::sync::{Arc, Mutex};

use fittings::{
    async_trait::async_trait,
    core::message::{JsonRpcId, Request, Response},
    Client, Connector, FittingsError, Service, ServiceContext, Transport,
};
use fittings_testkit::{fixtures::parse_response_fixture, memory_transport::MemoryTransport};
use serde_json::{json, Value};
use tokio::time::{timeout, Duration};

struct EchoService {
    seen: Arc<Mutex<Vec<(String, Value)>>>,
}

#[async_trait]
impl Service for EchoService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        self.seen
            .lock()
            .expect("seen mutex")
            .push((req.method.clone(), req.params.clone()));
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
async fn inbound_request_with_service_routes_to_handler() {
    let (client_transport, mut server_transport) = MemoryTransport::pair(16);
    let seen = Arc::new(Mutex::new(Vec::new()));
    let _client = Client::connect(OneShotConnector {
        transport: tokio::sync::Mutex::new(Some(client_transport)),
    })
    .await
    .expect("client should connect")
    .with_service(EchoService {
        seen: Arc::clone(&seen),
    });

    let request =
        b"{\"jsonrpc\":\"2.0\",\"id\":\"s_7\",\"method\":\"ping\",\"params\":{\"n\":1}}\n";
    server_transport
        .send(request)
        .await
        .expect("server should send peer-originated request");

    let frame = timeout(Duration::from_millis(500), server_transport.recv())
        .await
        .expect("client should answer before timeout")
        .expect("server should receive a response frame");

    let response = parse_response_fixture(&frame).expect("response should decode");
    assert_eq!(response.id, JsonRpcId::from("s_7"));
    assert!(
        response.error.is_none(),
        "successful response should carry no error envelope: {response:?}",
    );
    assert_eq!(response.result, Some(json!({ "echoed": { "n": 1 } })));

    let seen = seen.lock().expect("seen mutex").clone();
    assert_eq!(seen, vec![("ping".to_string(), json!({ "n": 1 }))]);
}
