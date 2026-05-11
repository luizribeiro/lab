mod common;

use common::confirm_state_kit::held;
use rafaello_core::bus::JsonRpcId;
use rafaello_core::gate::ConfirmState;

#[test]
fn mark_session_grant_requested_twice_is_idempotent() {
    let state = ConfirmState::new();
    let id = JsonRpcId::from("c-1");
    state.reserve(id.clone(), held());

    state
        .mark_session_grant_requested(&id)
        .expect("1st call Ok");
    state
        .mark_session_grant_requested(&id)
        .expect("2nd call Ok (no-op)");

    let (_, flag) = state.try_resolve(&id).expect("still Active");
    assert!(flag);
}
