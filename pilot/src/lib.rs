//! `pilot` drives headless AI coding-agent CLIs (claude, codex, gemini, pi)
//! over their stream-JSON modes.
//!
//! See the `Session`, `Driver`, and `Event` types for the public API.

mod error;

pub use error::{Error, ParseError, Result};
