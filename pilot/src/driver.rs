//! Driver abstraction: a `Driver` knows how to spawn a particular agent CLI
//! and parse its stream-JSON output into normalized [`Event`]s.

use std::path::PathBuf;
use std::time::Duration;

use secrecy::SecretString;
use uuid::Uuid;

use crate::{Event, ParseError};

pub mod claude;
pub mod gemini;
pub mod pi;

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningLevel {
    Low,
    Medium,
    High,
}

#[derive(Default, Debug, Clone)]
pub enum Auth {
    #[default]
    Ambient,
    ApiKey(SecretString),
}

#[derive(Debug, Default, Clone)]
pub struct TurnOptions {
    pub model: Option<String>,
    pub reasoning: Option<ReasoningLevel>,
    pub timeout: Option<Duration>,
    pub env: Vec<(String, String)>,
    pub raw_args: Vec<String>,
}

pub trait Driver: Send + Sync {
    fn name(&self) -> &'static str;

    /// Build the command for the FIRST turn of a session.
    fn command(&self, session_id: Uuid, prompt: &str, opts: &TurnOptions) -> CommandSpec;

    /// Build the command for a SUBSEQUENT turn that should resume an
    /// existing session. Default implementation delegates to `command` —
    /// drivers whose CLI requires distinct first-turn vs resume flags
    /// (e.g. gemini, codex) MUST override this.
    fn resume_command(&self, session_id: Uuid, prompt: &str, opts: &TurnOptions) -> CommandSpec {
        self.command(session_id, prompt, opts)
    }

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
