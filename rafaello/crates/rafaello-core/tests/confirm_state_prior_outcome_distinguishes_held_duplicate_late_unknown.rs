mod common;

use common::confirm_state_kit::held;
use rafaello_core::bus::JsonRpcId;
use rafaello_core::gate::{ConfirmState, PriorOutcome};

#[test]
fn prior_outcome_distinguishes_held_duplicate_late_unknown() {
    let state = ConfirmState::new();
    let held_id = JsonRpcId::from("c-held");
    let dup_id = JsonRpcId::from("c-dup");
    let late_id = JsonRpcId::from("c-late");
    let unknown_id = JsonRpcId::from("c-unknown");

    state.reserve(held_id.clone(), held());
    state.reserve(dup_id.clone(), held());
    state.reserve(late_id.clone(), held());

    assert!(state.try_resolve(&dup_id).is_some());
    assert!(state.try_take_for_timeout(&late_id).is_some());

    assert_eq!(state.prior_outcome(&held_id), PriorOutcome::Held);
    assert_eq!(state.prior_outcome(&dup_id), PriorOutcome::Duplicate);
    assert_eq!(state.prior_outcome(&late_id), PriorOutcome::Late);
    assert_eq!(state.prior_outcome(&unknown_id), PriorOutcome::Unknown);
}
