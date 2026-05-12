//! `rfl init` subcommand scaffold (scope §A1).
//!
//! This commit is the CLI-surface scaffold only: parses
//! `--project-root` / `--yes` / `--force`, honours the idempotency
//! invariant by short-circuiting when `${PROJECT_ROOT}/rafaello.lock`
//! already exists (without `--force`), and otherwise returns
//! `InitError::NotYetImplemented`. The default-lock TOML emit and
//! PP1 copy land in c02.

use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, clap::Args)]
pub struct InitArgs {
    #[arg(long, default_value_t = false)]
    pub yes: bool,
    #[arg(long, default_value_t = false)]
    pub force: bool,
    #[arg(long)]
    pub project_root: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("io error on {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("NotYetImplemented")]
    NotYetImplemented,
}

pub fn run(args: InitArgs) -> Result<(), InitError> {
    let project_root = match args.project_root {
        Some(p) => p,
        None => std::env::current_dir().map_err(|source| InitError::Io {
            path: PathBuf::from("."),
            source,
        })?,
    };
    let lock_path = project_root.join("rafaello.lock");
    if lock_path.exists() && !args.force {
        let mut stderr = std::io::stderr().lock();
        let _ = writeln!(stderr, "lock already present at {}", lock_path.display());
        let _ = stderr.flush();
        return Ok(());
    }
    Err(InitError::NotYetImplemented)
}
