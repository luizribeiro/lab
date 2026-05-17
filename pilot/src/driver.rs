//! Driver abstraction: a `Driver` knows how to spawn a particular agent CLI
//! and parse its stream-JSON output into normalized [`Event`]s.

use std::path::PathBuf;
use std::time::Duration;

use secrecy::SecretString;
use uuid::Uuid;

use crate::{Event, ParseError};

pub mod claude;
pub mod codex;
pub mod gemini;
pub mod pi;

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// Path overrides common to multiple agent CLIs. Each driver is
/// responsible for translating these into its CLI's actual mechanism
/// (flag or env var). Drivers that don't support a given override
/// must return `Error::UnsupportedOption` from `command()` /
/// `resume_command()` rather than silently ignoring it.
#[non_exhaustive]
#[derive(Default, Debug, Clone)]
pub struct AgentPaths {
    /// Override the CLI's config home directory.
    /// - claude:  `CLAUDE_CONFIG_DIR` env var
    /// - codex:   `CODEX_HOME` env var
    /// - pi:      `PI_CODING_AGENT_DIR` env var
    /// - gemini:  NOT SUPPORTED — setting this on a `GeminiConfig`
    ///   causes `command()` to return `Error::UnsupportedOption`
    pub config_home: Option<std::path::PathBuf>,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningLevel {
    Low,
    Medium,
    High,
}

#[non_exhaustive]
#[derive(Default, Debug, Clone)]
pub enum Auth {
    #[default]
    Ambient,
    ApiKey(SecretString),
}

#[non_exhaustive]
#[derive(Debug, Default, Clone)]
pub struct TurnOptions {
    pub model: Option<String>,
    pub reasoning: Option<ReasoningLevel>,
    pub timeout: Option<Duration>,
    pub env: Vec<(String, String)>,
    /// Additional CLI flags to append after the pilot-generated argv.
    ///
    /// Placement contract (the same for every built-in driver): these
    /// arguments are inserted AFTER the typed flags (`--model`,
    /// `--permission-mode`, `--sandbox`, etc.) and BEFORE the prompt
    /// positional. They are appended verbatim with no shell quoting.
    ///
    /// Use this as an escape hatch for CLI flags pilot doesn't yet
    /// expose as typed fields. If you find yourself reaching for it
    /// repeatedly for the same flag, that's a signal to open an issue
    /// asking for a typed knob.
    pub extra_args: Vec<String>,
}

/// Input to a single turn.
///
/// Currently has only the `Text` variant. Future variants will represent
/// multimodal inputs (images, files, structured tool responses) — the
/// enum is `#[non_exhaustive]` so callers must include a catch-all arm
/// when matching, and adding variants is a minor-version change.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum TurnInput {
    Text(String),
}

impl From<&str> for TurnInput {
    fn from(s: &str) -> Self {
        TurnInput::Text(s.to_string())
    }
}

impl From<&String> for TurnInput {
    fn from(s: &String) -> Self {
        TurnInput::Text(s.clone())
    }
}

impl From<String> for TurnInput {
    fn from(s: String) -> Self {
        TurnInput::Text(s)
    }
}

pub trait Driver: Send + Sync {
    fn name(&self) -> &'static str;

    /// Build the command for the FIRST turn of a session.
    fn command(
        &self,
        session_id: Uuid,
        input: &TurnInput,
        opts: &TurnOptions,
    ) -> crate::Result<CommandSpec>;

    /// Build the command for a SUBSEQUENT turn that should resume an
    /// existing session. Default implementation delegates to `command` —
    /// drivers whose CLI requires distinct first-turn vs resume flags
    /// (e.g. gemini, codex) MUST override this.
    fn resume_command(
        &self,
        session_id: Uuid,
        input: &TurnInput,
        opts: &TurnOptions,
    ) -> crate::Result<CommandSpec> {
        self.command(session_id, input, opts)
    }

    /// Inspect the raw JSON line emitted by the child BEFORE it is normalized
    /// by [`Driver::parse`]. Drivers with per-session state (e.g. codex,
    /// where the CLI auto-generates a thread_id on the first turn) can
    /// override this and use interior mutability (an `Arc<Mutex<...>>`
    /// field) to remember information for future `resume_command` calls.
    ///
    /// Default implementation: no-op. Drivers that don't need it pay nothing.
    fn observe(&self, _session_id: Uuid, _raw: &serde_json::Value) {}

    fn parse(&self, line: serde_json::Value) -> std::result::Result<Vec<Event>, ParseError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_apikey_redacts_in_debug() {
        let auth = Auth::ApiKey(SecretString::from("sk-secret".to_string()));
        let dbg = format!("{auth:?}");
        assert!(!dbg.contains("sk-secret"));
    }

    #[test]
    fn auth_default_is_ambient() {
        assert!(matches!(Auth::default(), Auth::Ambient));
    }
}
