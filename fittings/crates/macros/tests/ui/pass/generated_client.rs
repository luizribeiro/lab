use fittings::{FittingsError, async_trait};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct HelloParams {
    name: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct HelloResult {
    message: String,
}

#[fittings::service]
trait HelloService {
    async fn hello(&self, params: HelloParams) -> Result<HelloResult, FittingsError>;
}

struct DummyTransport;

#[async_trait::async_trait]
impl fittings::Transport for DummyTransport {
    async fn send(&mut self, _frame: &[u8]) -> Result<(), FittingsError> {
        Err(FittingsError::internal("unused"))
    }

    async fn recv(&mut self) -> Result<Vec<u8>, FittingsError> {
        Err(FittingsError::internal("unused"))
    }
}

struct DummyConnector;

#[async_trait::async_trait]
impl fittings::Connector for DummyConnector {
    type Connection = DummyTransport;

    async fn connect(&self) -> Result<Self::Connection, FittingsError> {
        Err(FittingsError::internal("unused"))
    }
}

fn main() {
    let _maybe_client: Option<HelloServiceClient<DummyConnector>> = None;
    let _connect = HelloServiceClient::<DummyConnector>::connect;
    let _spawn_future = HelloServiceClient::spawn("hello-service");
    let _spawn_with_config_future =
        HelloServiceClient::spawn_with_config("hello-service", fittings::serde_json::json!({}));
    let _hello = HelloServiceClient::<DummyConnector>::hello;
    let _main_future = DummyServiceImpl.main();
}

struct DummyServiceImpl;

impl HelloService for DummyServiceImpl {
    async fn hello(&self, _params: HelloParams) -> Result<HelloResult, FittingsError> {
        Ok(HelloResult {
            message: String::new(),
        })
    }
}

