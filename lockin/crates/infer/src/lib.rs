//! Policy-inference data model and runtime for lockin.
//!
//! This crate defines the cross-platform event types that observation
//! backends produce and that the policy compactor consumes to emit a
//! starter `lockin.toml`. Concrete platform backends (Linux syd,
//! macOS seatbelt) and the `lockin infer` CLI integration live in
//! later modules and commits.

pub mod event;
pub mod path;

pub use event::{DiagnosticLevel, FsOp, InferDiagnostic, InferEvent};
pub use path::canonicalize_observed;
