//! Driver abstraction: a `Driver` knows how to spawn a particular agent CLI
//! and parse its stream-JSON output into normalized [`Event`]s.

use std::path::PathBuf;
use std::time::Duration;

use secrecy::SecretString;
use uuid::Uuid;

use crate::{Event, ParseError};

pub mod claude;

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
    fn command(&self, session_id: Uuid, prompt: &str, opts: &TurnOptions) -> CommandSpec;
    fn parse(&self, line: serde_json::Value) -> std::result::Result<Event, ParseError>;
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
