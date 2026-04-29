//! Runtime denial tracing for lockin.
//!
//! Run a program under `lockin.toml` enforcement; report each access
//! the sandbox denied. Like `lockin infer` but for the enforcement
//! policy: tells you which rules your config is missing or which
//! accesses your program would have made.
//!
//! Built on top of [`lockin::SandboxBuilder`] with
//! [`lockin::ObservationMode::DenyTraceWithRunId`]: the renderer emits
//! the user's allow rules normally and replaces only the catch-all
//! `(deny default)` with a tagged report variant, so allowed accesses
//! stay quiet and denied ones surface as [`AccessAction::Deny`] events.

mod runner;

pub use runner::{trace, TraceOptions, TraceReport, TraceRequest};

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "macos")]
mod darwin;
