use fittings::{
    async_trait::async_trait,
    core::message::{JsonRpcId, Request, Response},
    Client, Connector, FittingsError, Server, Service, ServiceContext,
};
use fittings_testkit::memory_transport::MemoryTransport;
use serde_json::{json, Value};

struct VariantCase {
    name: &'static str,
    err: fn() -> FittingsError,
    expected_message: &'static str,
    expected_data: Value,
}

fn cases() -> [VariantCase; 5] {
    [
        VariantCase {
            name: "Parse",
            err: || FittingsError::parse_error_with_data("bad json", json!({"line": 1})),
            expected_message: "bad json",
            expected_data: json!({"line": 1}),
        },
        VariantCase {
            name: "InvalidRequest",
            err: || FittingsError::invalid_request_with_data("missing id", json!({"field": "id"})),
            expected_message: "missing id",
            expected_data: json!({"field": "id"}),
        },
        VariantCase {
            name: "MethodNotFound",
            err: || {
                FittingsError::method_not_found_with_data(
                    "math/double",
                    json!({"hint": "register first"}),
                )
            },
            expected_message: "math/double",
            expected_data: json!({"hint": "register first"}),
        },
        VariantCase {
            name: "InvalidParams",
            err: || {
                FittingsError::invalid_params_with_data(
                    "value must be positive",
                    json!({"field": "n", "got": -1}),
                )
            },
            expected_message: "value must be positive",
            expected_data: json!({"field": "n", "got": -1}),
        },
        VariantCase {
            name: "Internal",
            err: || {
                FittingsError::internal_with_data(
                    "downstream unavailable",
                    json!({"upstream": "rpc"}),
                )
            },
            expected_message: "downstream unavailable",
            expected_data: json!({"upstream": "rpc"}),
        },
    ]
}

struct ErroringService {
    err: fn() -> FittingsError,
}

#[async_trait]
impl Service for ErroringService {
    async fn call(&self, _req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        Err((self.err)())
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
async fn predefined_error_variants_round_trip_message_and_data_byte_equal() {
    for case in cases() {
        let service = ErroringService { err: case.err };

        let (client_transport, server_transport) = MemoryTransport::pair(16);
        let server = Server::new(service, server_transport);
        let server_handle = tokio::spawn(server.serve());

        let client = Client::connect(OneShotConnector {
            transport: tokio::sync::Mutex::new(Some(client_transport)),
        })
        .await
        .expect("client should connect");

        let error = client
            .call("any/method", json!({}))
            .await
            .expect_err("server returns an error");

        let (message, data) = match &error {
            FittingsError::Parse { message, data } => (message.clone(), data.clone()),
            FittingsError::InvalidRequest { message, data } => (message.clone(), data.clone()),
            FittingsError::MethodNotFound { message, data } => (message.clone(), data.clone()),
            FittingsError::InvalidParams { message, data } => (message.clone(), data.clone()),
            FittingsError::Internal { message, data } => (message.clone(), data.clone()),
            other => panic!("[{}] unexpected variant: {other:?}", case.name),
        };

        assert_eq!(
            message, case.expected_message,
            "[{}] message must round-trip byte-equal",
            case.name
        );
        assert_eq!(
            data,
            Some(case.expected_data.clone()),
            "[{}] data must round-trip byte-equal",
            case.name
        );

        // ensure the variant matches the source side
        let source = (case.err)();
        match (&source, &error) {
            (FittingsError::Parse { .. }, FittingsError::Parse { .. })
            | (FittingsError::InvalidRequest { .. }, FittingsError::InvalidRequest { .. })
            | (FittingsError::MethodNotFound { .. }, FittingsError::MethodNotFound { .. })
            | (FittingsError::InvalidParams { .. }, FittingsError::InvalidParams { .. })
            | (FittingsError::Internal { .. }, FittingsError::Internal { .. }) => {}
            _ => panic!(
                "[{}] decoded variant does not match source: source={source:?} decoded={error:?}",
                case.name
            ),
        }

        // we use `JsonRpcId` from the imported namespace to keep the path ergonomic
        let _: Option<JsonRpcId> = None;

        drop(client);
        let _ = server_handle.await.expect("server should join");
    }
}
