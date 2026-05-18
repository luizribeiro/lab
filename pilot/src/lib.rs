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

/// Construct a [`test_support::Cassette`] whose fixture path is auto-derived
/// from the enclosing test function's name. Expands to roughly:
///
/// ```ignore
/// Cassette::auto(inner, "<CARGO_MANIFEST_DIR>/tests/fixtures/recorded/<test_fn>.jsonl")
/// ```
///
/// Uses the `fn f() {} + type_name_of(f)` idiom to extract the calling
/// function's name at compile time without proc macros.
#[macro_export]
#[cfg(feature = "test-support")]
macro_rules! cassette {
    ($inner:expr) => {{
        fn _cassette_marker() {}
        fn _type_name_of<T>(_: T) -> &'static str {
            ::std::any::type_name::<T>()
        }
        let _full = _type_name_of(_cassette_marker);
        let _stripped = &_full[.._full.len() - "::_cassette_marker".len()];
        let _stripped = _stripped.strip_suffix("::{{closure}}").unwrap_or(_stripped);
        let _test_name = _stripped.rsplit("::").next().unwrap_or(_stripped);
        let _fixture: ::std::path::PathBuf = ::std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/recorded")
            .join(format!("{}.jsonl", _test_name));
        $crate::test_support::Cassette::auto($inner, _fixture)
    }};
}

pub use driver::claude::{Claude, ClaudeConfig, PermissionMode};
pub use driver::codex::{Codex, CodexConfig, CodexPilotState, SandboxMode};
pub use driver::gemini::{ApprovalMode, Gemini, GeminiConfig};
pub use driver::pi::{Pi, PiConfig, PiPilotState};
pub use driver::{AgentPaths, Auth, CommandSpec, Driver, ReasoningLevel, TurnInput, TurnOptions};
pub use error::{Error, ParseError, Result};
pub use event::Event;
pub use session::Session;
pub use turn::{Turn, TurnItem, TurnStream};
