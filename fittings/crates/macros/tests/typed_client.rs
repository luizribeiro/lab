use std::sync::Arc;

use fittings::{FittingsError, Transport};
use fittings_testkit::{fixtures::success_response_line, memory_transport::MemoryTransport};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct AddParams {
    left: i32,
    right: i32,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct AddResult {
    sum: i32,
}

#[fittings::service]
trait MathClientService {
    #[fittings::method(name = "math/add")]
    async fn add(&self, params: AddParams) -> Result<AddResult, FittingsError>;
}

#[derive(Debug, Deserialize, JsonSchema)]
struct FailingParams;

impl Serialize for FailingParams {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Err(serde::ser::Error::custom("cannot serialize params"))
    }
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct AckResult {
    ok: bool,
}

#[fittings::service]
trait EncodeFailureService {
    async fn fail_encode(&self, params: FailingParams) -> Result<AckResult, FittingsError>;
}

struct OneShotConnector {
    transport: Arc<Mutex<Option<MemoryTransport>>>,
}

impl OneShotConnector {
    fn new(transport: MemoryTransport) -> Self {
        Self {
            transport: Arc::new(Mutex::new(Some(transport))),
        }
    }
}

#[fittings::async_trait::async_trait]
impl fittings::Connector for OneShotConnector {
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
async fn generated_typed_client_roundtrips_method_calls() {
    let (client_transport, mut server_transport) = MemoryTransport::pair(8);
    let client = MathClientServiceClient::connect(OneShotConnector::new(client_transport))
        .await
        .expect("client should connect");

    let server = tokio::spawn(async move {
        let request_frame = server_transport.recv().await.expect("receive request");
        let request: fittings::RequestEnvelope = fittings::serde_json::from_slice(&request_frame)
            .expect("request envelope should decode");

        assert_eq!(request.method, "math/add");
        assert_eq!(
            request.params,
            Some(fittings::serde_json::json!({"left": 20, "right": 22}))
        );

        let response_line = success_response_line(
            request.id.as_ref().expect("request should carry an id"),
            fittings::serde_json::json!({"sum": 42}),
        )
        .expect("encode response");
        server_transport
            .send(&response_line)
            .await
            .expect("send response");
    });

    let result = client
        .add(AddParams {
            left: 20,
            right: 22,
        })
        .await
        .expect("typed method should succeed");

    assert_eq!(result.sum, 42);

    server.await.expect("server task should join");
}

#[tokio::test]
async fn generated_typed_client_maps_result_decode_failures_to_internal_error() {
    let (client_transport, mut server_transport) = MemoryTransport::pair(8);
    let client = MathClientServiceClient::connect(OneShotConnector::new(client_transport))
        .await
        .expect("client should connect");

    let server = tokio::spawn(async move {
        let request_frame = server_transport.recv().await.expect("receive request");
        let request: fittings::RequestEnvelope = fittings::serde_json::from_slice(&request_frame)
            .expect("request envelope should decode");

        let response_line = success_response_line(
            request.id.as_ref().expect("request should carry an id"),
            fittings::serde_json::json!({"sum": "not-an-int"}),
        )
        .expect("encode response");
        server_transport
            .send(&response_line)
            .await
            .expect("send malformed response");
    });

    let error = client
        .add(AddParams { left: 1, right: 2 })
        .await
        .expect_err("typed decode should fail");

    assert!(matches!(
        error,
        FittingsError::Internal(message)
            if message.contains("failed to decode result for method `math/add`")
    ));

    server.await.expect("server task should join");
}

#[tokio::test]
async fn generated_typed_client_maps_params_encode_failures_to_invalid_params() {
    let (client_transport, _server_transport) = MemoryTransport::pair(1);
    let client = EncodeFailureServiceClient::connect(OneShotConnector::new(client_transport))
        .await
        .expect("client should connect");

    let error = client
        .fail_encode(FailingParams)
        .await
        .expect_err("params encoding should fail");

    assert!(matches!(
        error,
        FittingsError::InvalidParams(message)
            if message.contains("failed to encode params for method `fail_encode`")
    ));
}
