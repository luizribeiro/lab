use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use tempo::config::Config;
use tempo::output::write_runs_to_path;
use tempo::runner::run_all;

#[derive(Debug, Parser)]
#[command(name = "tempo", version, about = "LLM inference throughput benchmark")]
struct Cli {
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Run {
        suite: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .with_writer(std::io::stderr)
            .init();
    }

    match cli.command {
        Command::Run { suite, output } => match run_command(&suite, &output).await {
            Ok(code) => code,
            Err(err) => {
                eprintln!("error: {err:#}");
                ExitCode::from(1)
            }
        },
    }
}

async fn run_command(suite_path: &Path, output_path: &Path) -> Result<ExitCode> {
    let toml_text = std::fs::read_to_string(suite_path)
        .with_context(|| format!("reading suite file {}", suite_path.display()))?;
    let config = Config::from_toml_str(&toml_text).context("parsing suite TOML")?;

    let result = run_all(&config).await.context("running suite")?;
    write_runs_to_path(output_path, &result.runs)
        .with_context(|| format!("writing results to {}", output_path.display()))?;

    eprintln!(
        "wrote {} rows to {} ({} cells, {} with zero successes)",
        result.runs.len(),
        output_path.display(),
        result.total_cells,
        result.zero_success_cells,
    );

    Ok(if result.zero_success_cells > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}
