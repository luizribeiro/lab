mod common;

use common::confirm_state_kit::held;
use rafaello_core::bus::JsonRpcId;
use rafaello_core::gate::ConfirmState;

#[test]
fn try_resolve_twice_returns_none_second_time() {
    let state = ConfirmState::new();
    let id = JsonRpcId::from("c-1");
    state.reserve(id.clone(), held());

    assert!(state.try_resolve(&id).is_some());
    assert!(state.try_resolve(&id).is_none(), "ResolvedByAnswer → None");
}
