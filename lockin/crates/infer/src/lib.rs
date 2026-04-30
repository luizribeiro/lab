//! Policy-inference data model and runtime for lockin.
//!
//! This crate defines the cross-platform event types that observation
//! backends produce and that the policy compactor consumes to emit a
//! starter `lockin.toml`. Concrete platform backends (Linux syd,
//! macOS seatbelt) and the `lockin infer` CLI integration live in
//! later modules and commits.

pub mod backend;
pub mod compact;
pub mod emit;
pub mod observe;

pub use backend::{BackendReport, InferBackend, InferRequest};
pub use compact::{compact, InferredPolicy};
pub use emit::{merge_into_config, render_toml, HEADER_COMMENT};
pub use lockin_observe::{
    canonicalize_event, canonicalize_observed, AccessAction, AccessEvent, DiagnosticLevel, FsOp,
    InferDiagnostic, InferEvent,
};
pub use observe::{infer, infer_with_backend, InferOptions, InferReport};
