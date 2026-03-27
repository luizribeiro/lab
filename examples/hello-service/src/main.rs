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
