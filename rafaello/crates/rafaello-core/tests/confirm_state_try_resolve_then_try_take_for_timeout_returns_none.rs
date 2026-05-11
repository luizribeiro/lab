mod common;

use common::confirm_state_kit::held;
use rafaello_core::bus::JsonRpcId;
use rafaello_core::gate::ConfirmState;

#[test]
fn try_resolve_then_try_take_for_timeout_returns_none() {
    let state = ConfirmState::new();
    let id = JsonRpcId::from("c-1");
    state.reserve(id.clone(), held());

    assert!(state.try_resolve(&id).is_some());
    assert!(
        state.try_take_for_timeout(&id).is_none(),
        "ResolvedByAnswer → None on timeout path"
    );
}
