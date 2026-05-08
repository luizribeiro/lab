pub mod mcp;

use fittings::{Server, Service, Transport};

pub const CANCELLATION_METHOD: &str = "notifications/cancelled";
pub const CANCELLATION_ID_FIELD: &str = "requestId";

pub fn configure_cancellation<S, T>(server: Server<S, T>) -> Server<S, T>
where
    S: Service + 'static,
    T: Transport + 'static,
{
    server.with_cancellation(CANCELLATION_METHOD, CANCELLATION_ID_FIELD)
}
