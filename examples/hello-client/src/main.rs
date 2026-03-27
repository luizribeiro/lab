use std::process;

use clap::Parser;
use fittings::TcpConnector;
use hello_api::{HelloParams, HelloServiceClient};

const DEFAULT_SERVICE_ADDRESS: &str = "127.0.0.1:7000";

#[derive(Debug, Parser)]
#[command(name = "hello-client", about = "Minimal TCP client for hello-service")]
struct Cli {
    #[arg(long = "addr", env = "HELLO_SERVICE_ADDR", default_value = DEFAULT_SERVICE_ADDRESS)]
    service_address: String,

    #[arg(default_value = "world")]
    name: String,
}

async fn request_hello_message(
    service_address: String,
    name: String,
) -> Result<String, fittings::FittingsError> {
    let client = HelloServiceClient::connect(TcpConnector::new(service_address)).await?;
    let result = client.hello(HelloParams { name }).await?;
    Ok(result.message)
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let message = request_hello_message(cli.service_address, cli.name).await?;
    println!("{message}");
    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(error) = run(Cli::parse()).await {
        eprintln!("{error}");
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;
    use fittings::{accept_one, FittingsError, RouterService, Server};
    use hello_api::{
        into_hello_service_router, HelloParams, HelloResult, HelloService, PingParams, PingResult,
    };
    use tokio::{
        net::TcpListener,
        time::{timeout, Duration},
    };

    use super::{request_hello_message, Cli, DEFAULT_SERVICE_ADDRESS};

    #[test]
    fn cli_defaults_addr_and_name() {
        let cli = Cli::parse_from(["hello-client"]);
        assert_eq!(cli.service_address, DEFAULT_SERVICE_ADDRESS);
        assert_eq!(cli.name, "world");
    }

    #[test]
    fn cli_accepts_addr_and_name() {
        let cli = Cli::parse_from(["hello-client", "--addr", "127.0.0.1:9000", "Ada"]);
        assert_eq!(cli.service_address, "127.0.0.1:9000");
        assert_eq!(cli.name, "Ada");
    }

    #[test]
    fn cli_rejects_unknown_flag() {
        let err = Cli::try_parse_from(["hello-client", "--unknown"]).expect_err("should fail");
        assert_eq!(err.kind(), clap::error::ErrorKind::UnknownArgument);
    }

    struct TestHelloService;

    impl HelloService for TestHelloService {
        async fn hello(&self, params: HelloParams) -> Result<HelloResult, FittingsError> {
            Ok(HelloResult {
                message: format!("Hello, {}!", params.name),
            })
        }

        async fn ping(&self, _params: PingParams) -> Result<PingResult, FittingsError> {
            Ok(PingResult { ok: true })
        }
    }

    #[tokio::test]
    async fn request_hello_message_uses_tcp_connector_based_client_flow() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let address = listener
            .local_addr()
            .expect("read listener address")
            .to_string();

        let server_task = tokio::spawn(async move {
            let transport = accept_one(&listener, 1_048_576)
                .await
                .expect("accept connection");
            let service = RouterService::new(into_hello_service_router(TestHelloService));
            Server::new(service, transport)
                .serve()
                .await
                .expect("serve request");
        });

        let message = request_hello_message(address, "Ada".to_string())
            .await
            .expect("request should succeed");
        assert_eq!(message, "Hello, Ada!");

        timeout(Duration::from_secs(2), server_task)
            .await
            .expect("server task should finish in time")
            .expect("server task should finish");
    }

    #[tokio::test]
    async fn request_hello_message_returns_error_when_address_is_unreachable() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let unreachable_address = listener
            .local_addr()
            .expect("read listener address")
            .to_string();
        drop(listener);

        let error = request_hello_message(unreachable_address, "Ada".to_string())
            .await
            .expect_err("request should fail");
        assert!(matches!(error, FittingsError::Transport(_)));
    }
}
