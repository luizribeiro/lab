//! Darwin trace backend — **stub**.
//!
//! Apple's `sandbox-exec` rejects the `(with report)` modifier on
//! `deny` actions ("report modifier does not apply to deny action"),
//! and a plain `(deny default)` denial is silent — no userland log
//! event is emitted for it. Kernel-side `Sandbox.kext` *does* publish
//! deny messages via `log stream` ("Sandbox: <proc>(<pid>) deny(N)
//! <op> <path>"), but they are not RUN_ID-tagged, so capturing them
//! requires filtering by the child's PID — a different shape from the
//! Linux side and from `lockin infer`'s existing per-RUN_ID drain.
//!
//! The right fix is a separate parser path for kernel sandbox lines
//! plus a way for `supervise_command` to expose the spawned child's
//! PID. Both are scope-creep for this commit; deferring to a follow-up.
//! Until then, `lockin trace` returns a clear unsupported error on
//! macOS so callers get a useful message rather than silent
//! mis-behavior.

use std::process::ExitStatus;

use anyhow::Result;
use lockin_infer::{AccessEvent, InferDiagnostic};

use crate::runner::TraceRequest;

pub(crate) fn run(
    _request: &TraceRequest,
) -> Result<(ExitStatus, Vec<AccessEvent>, Vec<InferDiagnostic>)> {
    anyhow::bail!(
        "lockin trace is not yet supported on macOS: Apple's sandbox-exec rejects \
         `(with report)` on deny actions, and `(deny default)` is silent. \
         Tracking follow-up; use Linux for trace today."
    )
}
