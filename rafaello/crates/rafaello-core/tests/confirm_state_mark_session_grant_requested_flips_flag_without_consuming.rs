mod common;

use common::confirm_state_kit::held;
use rafaello_core::bus::JsonRpcId;
use rafaello_core::gate::ConfirmState;

#[test]
fn mark_session_grant_requested_flips_flag_without_consuming() {
    let state = ConfirmState::new();
    let id = JsonRpcId::from("c-1");
    state.reserve(id.clone(), held());

    state
        .mark_session_grant_requested(&id)
        .expect("Active → Ok");

    assert!(state.is_held(&id), "entry is still Active after mark");
    let (_, flag) = state.try_resolve(&id).expect("still Active");
    assert!(flag, "session_grant_requested carried into try_resolve");
}
