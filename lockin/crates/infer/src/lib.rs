//! Policy-inference data model and runtime for lockin.
//!
//! This crate defines the cross-platform event types that observation
//! produces and that the policy compactor consumes to emit a starter
//! `lockin.toml`.

pub mod compact;
pub mod emit;
pub mod observe;

pub use compact::{compact, InferredPolicy};
pub use emit::{merge_into_config, render_toml, HEADER_COMMENT};
pub use lockin_observe::{
    canonicalize_event, canonicalize_observed, AccessAction, AccessEvent, DiagnosticLevel, FsOp,
    InferDiagnostic, InferEvent,
};
pub use observe::{infer, BackendReport, InferOptions, InferReport, InferRequest};
