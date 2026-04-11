use clap::Parser;
use fittings::TcpConnector;
use hello_api::{HelloParams, HelloServiceClient};

const DEFAULT_SERVICE_ADDRESS: &str = "127.0.0.1:7000";

#[derive(Debug, Parser)]
#[command(name = "hello-client", about = "Minimal TCP client for hello-service")]
struct Cli {
    #[arg(long = "addr", env = "HELLO_SERVICE_ADDR", default_value = DEFAULT_SERVICE_ADDRESS)]
    addr: String,

    #[arg(default_value = "world")]
    name: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let client = HelloServiceClient::connect(TcpConnector::new(cli.addr)).await?;
    let result = client.hello(HelloParams { name: cli.name }).await?;

    println!("{}", result.message);
    Ok(())
}
