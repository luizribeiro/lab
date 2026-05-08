//! c13 acceptance: a `Client` with no `with_service` registered answers
//! peer-originated inbound requests with `-32601 Method not found`.
//! Mirror of the server's existing dispatcher behaviour for unknown methods.

use fittings::{
    async_trait::async_trait, core::message::JsonRpcId, Client, Connector, FittingsError, Transport,
};
use fittings_testkit::{fixtures::parse_response_fixture, memory_transport::MemoryTransport};
use tokio::time::{timeout, Duration};

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
async fn inbound_request_without_service_returns_method_not_found() {
    let (client_transport, mut server_transport) = MemoryTransport::pair(16);
    let _client = Client::connect(OneShotConnector {
        transport: tokio::sync::Mutex::new(Some(client_transport)),
    })
    .await
    .expect("client should connect");

    let request = b"{\"jsonrpc\":\"2.0\",\"id\":\"s_1\",\"method\":\"missing\",\"params\":{}}\n";
    server_transport
        .send(request)
        .await
        .expect("server should send peer-originated request");

    let frame = timeout(Duration::from_millis(500), server_transport.recv())
        .await
        .expect("client should answer before timeout")
        .expect("server should receive a response frame");

    let response = parse_response_fixture(&frame).expect("response should decode");
    assert_eq!(response.id, JsonRpcId::from("s_1"));
    let error = response
        .error
        .expect("response must carry an error envelope");
    assert_eq!(error.code, -32601);
}
