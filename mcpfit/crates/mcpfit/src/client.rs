use fittings::{client::Client as FittingsClient, core::transport::Connector};

use crate::error::McpfitError;

pub struct Client<C>
where
    C: Connector + Send + Sync + 'static,
{
    #[allow(dead_code)]
    inner: FittingsClient<C>,
}

impl<C> Client<C>
where
    C: Connector + Send + Sync + 'static,
{
    pub async fn connect_uninitialized(connector: C) -> Result<Self, McpfitError> {
        let inner = FittingsClient::connect(connector)
            .await
            .map_err(|e| McpfitError::internal(format!("fittings connect: {e}")))?;
        Ok(Self { inner })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use fittings::{
        async_trait::async_trait,
        core::{error::FittingsError, transport::Connector},
    };
    use fittings_testkit::memory_transport::MemoryTransport;
    use tokio::sync::Mutex;

    use super::Client;

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

    #[tokio::test]
    async fn connect_uninitialized_does_not_send_any_handshake() {
        use std::time::Duration;

        use fittings::core::transport::Transport;
        use tokio::time::timeout;

        let (client_transport, mut server_transport) = MemoryTransport::pair(8);
        let _client = Client::connect_uninitialized(OneShotConnector::new(client_transport))
            .await
            .expect("client should connect without handshake");

        let waited = timeout(Duration::from_millis(50), server_transport.recv()).await;
        assert!(
            waited.is_err(),
            "connect_uninitialized must not write any MCP traffic, got: {:?}",
            waited.ok()
        );
    }

    #[tokio::test]
    async fn connect_uninitialized_surfaces_connector_failure() {
        struct FailingConnector;

        #[async_trait]
        impl Connector for FailingConnector {
            type Connection = MemoryTransport;

            async fn connect(&self) -> Result<Self::Connection, FittingsError> {
                Err(FittingsError::transport("simulated connect failure"))
            }
        }

        let err = match Client::connect_uninitialized(FailingConnector).await {
            Ok(_) => panic!("failing connector should propagate as McpfitError"),
            Err(e) => e,
        };
        assert!(
            err.to_string().contains("simulated connect failure"),
            "unexpected error: {err}"
        );
    }
}
