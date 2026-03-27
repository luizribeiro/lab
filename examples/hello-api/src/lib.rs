use fittings::FittingsError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HelloParams {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct HelloResult {
    pub message: String,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PingParams {}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct PingResult {
    pub ok: bool,
}

#[fittings::service]
pub trait HelloService {
    /// Greets the provided name
    async fn hello(&self, params: HelloParams) -> Result<HelloResult, FittingsError>;

    /// Health check
    async fn ping(&self, params: PingParams) -> Result<PingResult, FittingsError>;
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fittings::{async_trait::async_trait, Connector, RouterService, Server};
    use fittings_testkit::memory_transport::MemoryTransport;
    use tokio::sync::Mutex;

    use super::*;

    struct StubHelloService;

    impl HelloService for StubHelloService {
        async fn hello(&self, params: HelloParams) -> Result<HelloResult, FittingsError> {
            Ok(HelloResult {
                message: format!("Hello, {}!", params.name),
            })
        }

        async fn ping(&self, _params: PingParams) -> Result<PingResult, FittingsError> {
            Ok(PingResult { ok: true })
        }
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

    #[test]
    fn generated_symbols_are_usable_for_server_and_client_consumers() {
        let schema = hello_service_schema();
        assert_eq!(schema.name, "hello-service");

        let _router = into_hello_service_router(StubHelloService);
        let _maybe_client: Option<HelloServiceClient<fittings::ProcessConnector>> = None;
    }

    #[tokio::test]
    async fn service_can_be_tested_in_memory_via_generated_router_and_client() {
        let (client_transport, server_transport) = MemoryTransport::pair(16);

        let service = RouterService::new(into_hello_service_router(StubHelloService));
        let server = Server::new(service, server_transport);
        let server_task = tokio::spawn(async move { server.serve().await });

        let client = HelloServiceClient::connect(OneShotConnector::new(client_transport))
            .await
            .expect("client should connect");

        let hello = client
            .hello(HelloParams {
                name: "Ada".to_string(),
            })
            .await
            .expect("hello should succeed");
        assert_eq!(hello.message, "Hello, Ada!");

        let ping = client
            .ping(PingParams {})
            .await
            .expect("ping should succeed");
        assert!(ping.ok);

        drop(client);
        let result = server_task.await.expect("server task should join");
        assert!(result.is_ok());
    }
}
