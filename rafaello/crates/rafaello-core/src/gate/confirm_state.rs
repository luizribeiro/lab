//! `ConfirmState` — the named shared held-confirmation map
//! (scope §CG1a). Constructed by `rfl chat`, wrapped in `Arc`,
//! and cloned into both the gate (CG2/CG4/CG5) and the re-emit
//! pipeline's `confirm_answer` arm.
//!
//! Re-emit and the gate share a single coherent map: re-emit
//! validates + classifies + (for `always_allow_session`) flips
//! the grant-requested flag; the gate alone consumes the held
//! entry via `try_resolve` (answer path) or
//! `try_take_for_timeout` (deadline path).
//!
//! Compile-time policy: `re_hold` was removed entirely in
//! pi-3 M-3 (malformed-answer validation now happens before any
//! state mutation). The following doc-test must keep failing to
//! compile; if `re_hold` is ever re-introduced, this will start
//! to compile and the doc-test will fail loudly.
//!
//! ```compile_fail
//! use rafaello_core::bus::JsonRpcId;
//! use rafaello_core::gate::ConfirmState;
//!
//! fn must_not_compile(state: &ConfirmState, id: &JsonRpcId) {
//!     let _ = state.re_hold(id);
//! }
//! ```

use std::collections::HashMap;
use std::time::Instant;

use parking_lot::Mutex;

use crate::bus::{BusEvent, JsonRpcId};
use crate::lock::canonical_id::CanonicalId;

#[derive(Debug)]
pub struct ConfirmState {
    inner: Mutex<HashMap<JsonRpcId, HeldEntry>>,
}

#[derive(Debug)]
#[allow(clippy::large_enum_variant)] // scope §CG1a shape; Box would obscure the held entry.
enum HeldEntry {
    Active {
        held: HeldConfirmation,
        session_grant_requested: bool,
    },
    ResolvedByAnswer,
    TimedOut,
}

#[derive(Debug, Clone)]
pub struct HeldConfirmation {
    pub tool_request: BusEvent,
    pub deadline: Instant,
    pub dispatch_target: CanonicalId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorOutcome {
    Held,
    Duplicate,
    Late,
    Unknown,
}

#[derive(Debug, thiserror::Error)]
pub enum MarkError {
    #[error("entry not active")]
    NotActive,
}

impl ConfirmState {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub fn reserve(&self, confirm_id: JsonRpcId, held: HeldConfirmation) {
        let mut guard = self.inner.lock();
        if guard.contains_key(&confirm_id) {
            panic!(
                "ConfirmState::reserve called with colliding confirm_id; \
                 gate must allocate a fresh JsonRpcId per call"
            );
        }
        guard.insert(
            confirm_id,
            HeldEntry::Active {
                held,
                session_grant_requested: false,
            },
        );
    }

    pub fn is_held(&self, confirm_id: &JsonRpcId) -> bool {
        let guard = self.inner.lock();
        matches!(guard.get(confirm_id), Some(HeldEntry::Active { .. }))
    }

    pub fn mark_session_grant_requested(&self, confirm_id: &JsonRpcId) -> Result<(), MarkError> {
        let mut guard = self.inner.lock();
        match guard.get_mut(confirm_id) {
            Some(HeldEntry::Active {
                session_grant_requested,
                ..
            }) => {
                *session_grant_requested = true;
                Ok(())
            }
            _ => Err(MarkError::NotActive),
        }
    }

    pub fn try_resolve(&self, confirm_id: &JsonRpcId) -> Option<(HeldConfirmation, bool)> {
        let mut guard = self.inner.lock();
        let entry = guard.get_mut(confirm_id)?;
        if !matches!(entry, HeldEntry::Active { .. }) {
            return None;
        }
        let taken = std::mem::replace(entry, HeldEntry::ResolvedByAnswer);
        match taken {
            HeldEntry::Active {
                held,
                session_grant_requested,
            } => Some((held, session_grant_requested)),
            _ => unreachable!("checked Active above"),
        }
    }

    pub fn try_take_for_timeout(&self, confirm_id: &JsonRpcId) -> Option<HeldConfirmation> {
        let mut guard = self.inner.lock();
        let entry = guard.get_mut(confirm_id)?;
        if !matches!(entry, HeldEntry::Active { .. }) {
            return None;
        }
        let taken = std::mem::replace(entry, HeldEntry::TimedOut);
        match taken {
            HeldEntry::Active { held, .. } => Some(held),
            _ => unreachable!("checked Active above"),
        }
    }

    pub fn prior_outcome(&self, confirm_id: &JsonRpcId) -> PriorOutcome {
        let guard = self.inner.lock();
        match guard.get(confirm_id) {
            Some(HeldEntry::Active { .. }) => PriorOutcome::Held,
            Some(HeldEntry::ResolvedByAnswer) => PriorOutcome::Duplicate,
            Some(HeldEntry::TimedOut) => PriorOutcome::Late,
            None => PriorOutcome::Unknown,
        }
    }
}

impl Default for ConfirmState {
    fn default() -> Self {
        Self::new()
    }
}
