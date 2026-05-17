//! `pilot` drives headless AI coding-agent CLIs (claude, codex, gemini, pi)
//! over their stream-JSON modes.
//!
//! See the `Session`, `Driver`, and `Event` types for the public API.

mod driver;
mod error;
mod event;
mod process;
mod session;
mod turn;

#[cfg(feature = "test-support")]
pub mod test_support;

pub use driver::claude::{Claude, ClaudeConfig, PermissionMode};
pub use driver::codex::{Codex, CodexConfig, CodexPilotState, SandboxMode};
pub use driver::gemini::{ApprovalMode, Gemini, GeminiConfig};
pub use driver::pi::{Pi, PiConfig, PiPilotState};
pub use driver::{AgentPaths, Auth, CommandSpec, Driver, ReasoningLevel, TurnOptions};
pub use error::{Error, ParseError, Result};
pub use event::Event;
pub use session::Session;
pub use turn::{Turn, TurnItem, TurnStream};

/// Construct a built-in driver by name. Returns
/// [`Error::UnknownAgent`] if the name is not registered.
///
/// Names are matched case-sensitively against the value returned by
/// [`Driver::name()`]. Currently registered:
///   - `"claude"` — Anthropic Claude Code (`claude` CLI)
///   - `"codex"` — OpenAI Codex (`codex` CLI; first-turn only, see [`Codex`])
///   - `"gemini"` — Google Gemini (`gemini` CLI)
///   - `"pi"` — Inflection Pi (`pi` CLI)
pub fn driver(name: &str) -> Result<std::sync::Arc<dyn Driver>> {
    match name {
        "claude" => Ok(std::sync::Arc::new(driver::claude::Claude::new())),
        "codex" => Ok(std::sync::Arc::new(driver::codex::Codex::new())),
        "gemini" => Ok(std::sync::Arc::new(driver::gemini::Gemini::new())),
        "pi" => Ok(std::sync::Arc::new(driver::pi::Pi::new())),
        _ => Err(Error::UnknownAgent(name.to_string())),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn driver_claude_returns_named_claude() {
        let d = super::driver("claude").expect("registered");
        assert_eq!(d.name(), "claude");
    }

    #[test]
    fn driver_codex_returns_named_codex() {
        let d = super::driver("codex").expect("registered");
        assert_eq!(d.name(), "codex");
    }

    #[test]
    fn driver_gemini_returns_named_gemini() {
        let d = super::driver("gemini").expect("registered");
        assert_eq!(d.name(), "gemini");
    }

    #[test]
    fn driver_pi_returns_named_pi() {
        let d = super::driver("pi").expect("registered");
        assert_eq!(d.name(), "pi");
    }

    #[test]
    fn driver_unknown_returns_unknown_agent_error() {
        let result = super::driver("nonexistent");
        assert!(matches!(result, Err(super::Error::UnknownAgent(ref s)) if s == "nonexistent"));
    }

    #[test]
    fn driver_is_case_sensitive() {
        // Match against Driver::name() exactly. "Claude" should NOT match "claude".
        assert!(super::driver("Claude").is_err());
    }
}
