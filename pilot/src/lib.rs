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
pub use driver::{Auth, CommandSpec, Driver, ReasoningLevel, TurnOptions};
pub use error::{Error, ParseError, Result};
pub use event::Event;
pub use session::Session;
pub use turn::{Turn, TurnItem, TurnStream};
