//! `rfl status` subcommand (scope §Tr3 + §OP6).
//!
//! Prints one row per lock plugin with canonical id, bindings
//! summary, and active flags. The `flags.i_know_what_im_doing`
//! override is surfaced loudly (red ANSI on TTY, `[OVERRIDE]`
//! prefix otherwise) per security RFC §7.1; any-bundle
//! `GrantEnv.allow_secrets` is surfaced as a *distinct* yellow
//! suffix (`[SECRET: ...]` non-TTY) per §A11 — distinct from
//! the red panic marker.

use std::collections::BTreeSet;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use rafaello_core::error::LockError;
use rafaello_core::lock::{Bindings, Lock, PluginEntry};

#[derive(Debug, thiserror::Error)]
pub enum StatusError {
    #[error("io error on {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("lock parse error: {0}")]
    LockParse(#[source] Box<LockError>),
    #[error("io: {0}")]
    Stdout(#[source] std::io::Error),
}

pub fn run() -> Result<(), StatusError> {
    let project_root = std::env::current_dir().map_err(|source| StatusError::Io {
        path: PathBuf::from("."),
        source,
    })?;
    let lock_path = project_root.join("rafaello.lock");
    let raw = std::fs::read_to_string(&lock_path).map_err(|source| StatusError::Io {
        path: lock_path.clone(),
        source,
    })?;
    let lock = Lock::from_toml(&raw).map_err(|e| StatusError::LockParse(Box::new(e)))?;
    let mut stdout = std::io::stdout().lock();
    for (canonical, entry) in &lock.plugins {
        writeln!(
            stdout,
            "{}",
            render_row(&canonical.to_string(), entry, tty())
        )
        .map_err(StatusError::Stdout)?;
    }
    Ok(())
}

fn tty() -> bool {
    if std::env::var_os("RFL_STATUS_FORCE_TTY").is_some() {
        return true;
    }
    if std::env::var_os("RFL_STATUS_FORCE_NO_TTY").is_some() {
        return false;
    }
    std::io::stdout().is_terminal()
}

fn render_row(canonical: &str, entry: &PluginEntry, is_tty: bool) -> String {
    let secrets: BTreeSet<&str> = entry
        .grant
        .bundles
        .values()
        .filter_map(|b| b.env.as_ref())
        .flat_map(|e| e.allow_secrets.iter().map(String::as_str))
        .collect();
    let names: Vec<&str> = secrets.into_iter().collect();
    let (prefix, id) = if entry.flags.i_know_what_im_doing {
        if is_tty {
            (String::new(), format!("\x1b[31m{canonical}\x1b[0m"))
        } else {
            ("[OVERRIDE] ".to_string(), canonical.to_string())
        }
    } else {
        (String::new(), canonical.to_string())
    };
    let suffix = if names.is_empty() {
        String::new()
    } else if is_tty {
        format!(" \x1b[33mexplicit secret: {}\x1b[0m", names.join(", "))
    } else {
        format!(" [SECRET: {}]", names.join(", "))
    };
    let bindings = format_bindings(&entry.bindings);
    let sep = if bindings.is_empty() { "" } else { " " };
    format!("{prefix}{id}{sep}{bindings}{suffix}")
}

fn format_bindings(b: &Bindings) -> String {
    let mut parts: Vec<String> = Vec::new();
    if b.provider {
        parts.push("provider".to_string());
    }
    if !b.tools.is_empty() {
        parts.push(format!("tools=[{}]", b.tools.join(",")));
    }
    if !b.renderer_kinds.is_empty() {
        parts.push(format!("renderers=[{}]", b.renderer_kinds.join(",")));
    }
    parts.join(" ")
}
