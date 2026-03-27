use std::process;

use fittings::FittingsError;
use hello_api::{HelloParams, HelloResult, HelloService, PingParams, PingResult};

struct HelloServiceImpl;

impl HelloService for HelloServiceImpl {
    async fn hello(&self, params: HelloParams) -> Result<HelloResult, FittingsError> {
        Ok(HelloResult {
            message: format!("Hello, {}!", params.name),
        })
    }

    async fn ping(&self, _params: PingParams) -> Result<PingResult, FittingsError> {
        Ok(PingResult { ok: true })
    }
}

#[tokio::main]
async fn main() {
    process::exit(HelloServiceImpl.main().await);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fittings::{async_trait::async_trait, Connector, RouterService, Server};
    use fittings_testkit::memory_transport::MemoryTransport;
    use hello_api::{into_hello_service_router, HelloParams, HelloServiceClient, PingParams};
    use tokio::sync::Mutex;

    use super::HelloServiceImpl;

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

        async fn connect(&self) -> Result<Self::Connection, fittings::FittingsError> {
            self.transport
                .lock()
                .await
                .take()
                .ok_or_else(|| fittings::FittingsError::internal("connector already used"))
        }
    }

    #[tokio::test]
    async fn service_methods_can_be_tested_in_memory_through_generated_client() {
        let (client_transport, server_transport) = MemoryTransport::pair(16);

        let service = RouterService::new(into_hello_service_router(HelloServiceImpl));
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
