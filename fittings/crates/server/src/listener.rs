use std::sync::Arc;

use fittings_core::{error::FittingsError, service::Service, transport::Listener};

use crate::Server;

pub async fn serve_listener<S, L>(
    service: Arc<S>,
    listener: L,
    max_in_flight: usize,
) -> Result<(), FittingsError>
where
    S: Service + 'static,
    L: Listener + Send + Sync + 'static,
    L::Connection: Send + Sync + 'static,
{
    loop {
        let connection = listener.accept().await?;
        let service = Arc::clone(&service);

        tokio::spawn(async move {
            let _ = Server::new(service, connection)
                .with_max_in_flight(max_in_flight)
                .serve()
                .await;
        });
    }
}
