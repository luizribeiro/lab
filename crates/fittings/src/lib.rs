pub const FITTINGS_PROTOCOL_VERSION: &str = "1";

pub use async_trait;
pub use schemars;
pub use serde_json;

pub use fittings_core as core;
pub use fittings_server as server;
pub use fittings_spawn as spawn;
pub use fittings_transport as transport;
pub use fittings_wire as wire;

pub use fittings_core::{
    error::FittingsError,
    message::{Metadata, Request, Response, ServiceError},
    middleware::Middleware,
    service::Service,
    transport::{Connector, Listener, Transport},
};
pub use fittings_macros::service;
pub use fittings_server::{MethodRouter, RouterService, Server};
pub use fittings_spawn::{
    detect_mode, parse_server_config, validate_service_schema, ConfigError, MethodSchema,
    RunOutcome, SchemaValidationError, ServiceSchema, SpawnMode, SpawnModeError, SpawnRunner,
};
pub use fittings_transport::stdio::{from_process_stdio, StdioTransport};
pub use fittings_wire::{
    codec::{decode_request_line, encode_response_line, WireDecodeError, WireEncodeError},
    error_map::to_error_envelope,
    types::{ErrorEnvelope, RequestEnvelope, ResponseEnvelope},
};

#[cfg(test)]
mod tests {
    use super::{
        decode_request_line, encode_response_line, to_error_envelope, FittingsError,
        ResponseEnvelope, RunOutcome, SchemaValidationError, SpawnMode, FITTINGS_PROTOCOL_VERSION,
    };

    #[test]
    fn re_exports_expose_expected_symbols() {
        assert_eq!(FITTINGS_PROTOCOL_VERSION, "1");

        let request = decode_request_line(br#"{"id":"1","method":"ping","params":{}}"#)
            .expect("request should decode");
        assert_eq!(request.id, "1");
        assert_eq!(request.method, "ping");

        let success = ResponseEnvelope::success("1", request.params.clone(), Default::default());
        let encoded = encode_response_line(&success).expect("response should encode");
        assert!(encoded.ends_with(b"\n"));

        let mapped = to_error_envelope("req".to_string(), FittingsError::invalid_request("bad"));
        assert_eq!(mapped.error.expect("error").code, -32600);

        let _mode = SpawnMode::Schema;
        let _outcome = RunOutcome::Exit(0);
        let _schema_error: Option<SchemaValidationError> = None;
    }
}
