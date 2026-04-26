use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use tempo::config::Config;
use tempo::output::write_runs_to_path;
use tempo::progress::{IndicatifReporter, NoopReporter, ProgressReporter};
use tempo::report::render_report_from_path;
use tempo::runner::run_all;
use tempo::stats;
use tempo::summary;

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
        output: Option<PathBuf>,
        #[arg(long)]
        no_summary: bool,
        #[arg(long)]
        no_progress: bool,
    },
    Report {
        results: PathBuf,
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
        Command::Run {
            suite,
            output,
            no_summary,
            no_progress,
        } => match run_command(&suite, output.as_deref(), no_summary, no_progress).await {
            Ok(code) => code,
            Err(err) => {
                eprintln!("error: {err:#}");
                ExitCode::from(1)
            }
        },
        Command::Report { results } => match report_command(&results) {
            Ok(code) => code,
            Err(err) => {
                eprintln!("error: {err:#}");
                ExitCode::from(1)
            }
        },
    }
}

fn color_enabled(is_tty: bool) -> bool {
    is_tty && std::env::var_os("NO_COLOR").is_none()
}

fn report_command(results_path: &Path) -> Result<ExitCode> {
    let table =
        render_report_from_path(results_path, color_enabled(std::io::stdout().is_terminal()))?;
    println!("{table}");
    Ok(ExitCode::SUCCESS)
}

async fn run_command(
    suite_path: &Path,
    output_path: Option<&Path>,
    no_summary: bool,
    no_progress: bool,
) -> Result<ExitCode> {
    let toml_text = std::fs::read_to_string(suite_path)
        .with_context(|| format!("reading suite file {}", suite_path.display()))?;
    let config = Config::from_toml_str(&toml_text).context("parsing suite TOML")?;

    let owned_temp_path;
    let output_path = match output_path {
        Some(p) => p,
        None => {
            owned_temp_path = std::env::temp_dir().join(format!(
                "tempo-{}.json",
                chrono::Utc::now().format("%Y%m%dT%H%M%S")
            ));
            owned_temp_path.as_path()
        }
    };

    let reporter: Box<dyn ProgressReporter> = if !no_progress && std::io::stderr().is_terminal() {
        Box::new(IndicatifReporter::new())
    } else {
        Box::new(NoopReporter)
    };

    let result = run_all(&config, reporter.as_ref())
        .await
        .context("running suite")?;
    write_runs_to_path(output_path, &result.runs)
        .with_context(|| format!("writing results to {}", output_path.display()))?;

    eprintln!(
        "wrote {} rows to {}",
        result.runs.len(),
        output_path.display(),
    );
    if result.zero_success_cells > 0 {
        eprintln!(
            "{}/{} cells succeeded ({} failed)",
            result.total_cells - result.zero_success_cells,
            result.total_cells,
            result.zero_success_cells,
        );
    } else {
        eprintln!(
            "{}/{} cells succeeded",
            result.total_cells, result.total_cells
        );
    }

    if !no_summary {
        let color = color_enabled(std::io::stderr().is_terminal());
        let cell_stats = stats::aggregate(&result.runs);
        let table = summary::render(&cell_stats, color);
        eprintln!("{table}");
    }

    Ok(if result.zero_success_cells > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    })
}
