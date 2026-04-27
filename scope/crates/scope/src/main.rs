use std::process::ExitCode;

use clap::Parser;

use scope::cli::{Cli, Command};
use scope::config::Config;
use scope::handler;
use scope::runtime::Scope;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> anyhow::Result<String> {
    let cli = Cli::parse();
    let config = Config::load(cli.config.as_deref())?;
    let scope = Scope::from_config(&config)?;
    match cli.command {
        Command::Read { url, provider } => {
            handler::run_read(&scope, &url, provider.as_deref()).await
        }
        Command::Search { query, provider, limit } => {
            handler::run_search(&scope, &query, provider.as_deref(), limit).await
        }
        Command::Providers { kind } => Ok(handler::run_providers(&scope, kind)),
    }
}
