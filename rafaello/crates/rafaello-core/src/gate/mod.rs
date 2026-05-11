//! Confirmation gate — shared `ConfirmState` map (scope §CG1a)
//! plus future gate task (§CG2–CG5, later commits).

pub mod confirm_state;

pub use confirm_state::{ConfirmState, HeldConfirmation, MarkError, PriorOutcome};
