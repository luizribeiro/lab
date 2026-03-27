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

    #[test]
    fn generated_symbols_are_usable_for_server_and_client_consumers() {
        let schema = hello_service_schema();
        assert_eq!(schema.name, "hello-service");

        let _router = into_hello_service_router(StubHelloService);
        let _maybe_client: Option<HelloServiceClient<fittings::ProcessConnector>> = None;
    }
}
