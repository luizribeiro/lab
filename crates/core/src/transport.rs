use async_trait::async_trait;

use crate::error::FittingsError;

#[async_trait]
pub trait Transport: Send {
    async fn send(&mut self, frame: &[u8]) -> Result<(), FittingsError>;
    async fn recv(&mut self) -> Result<Vec<u8>, FittingsError>;
}

#[async_trait]
pub trait Connector: Send + Sync {
    type Connection: Transport;

    async fn connect(&self) -> Result<Self::Connection, FittingsError>;
}

#[async_trait]
pub trait Listener: Send + Sync {
    type Connection: Transport;

    async fn accept(&self) -> Result<Self::Connection, FittingsError>;
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use crate::error::FittingsError;

    use super::{Connector, Listener, Transport};

    struct DummyTransport;

    #[async_trait]
    impl Transport for DummyTransport {
        async fn send(&mut self, _frame: &[u8]) -> Result<(), FittingsError> {
            Ok(())
        }

        async fn recv(&mut self) -> Result<Vec<u8>, FittingsError> {
            Ok(Vec::new())
        }
    }

    struct DummyConnector;

    #[async_trait]
    impl Connector for DummyConnector {
        type Connection = DummyTransport;

        async fn connect(&self) -> Result<Self::Connection, FittingsError> {
            Ok(DummyTransport)
        }
    }

    struct DummyListener;

    #[async_trait]
    impl Listener for DummyListener {
        type Connection = DummyTransport;

        async fn accept(&self) -> Result<Self::Connection, FittingsError> {
            Ok(DummyTransport)
        }
    }

    fn assert_transport_impl<T: Transport>() {}
    fn assert_connector_impl<T: Connector>() {}
    fn assert_listener_impl<T: Listener>() {}

    #[test]
    fn transport_traits_are_implementable() {
        assert_transport_impl::<DummyTransport>();
        assert_connector_impl::<DummyConnector>();
        assert_listener_impl::<DummyListener>();
    }
}
