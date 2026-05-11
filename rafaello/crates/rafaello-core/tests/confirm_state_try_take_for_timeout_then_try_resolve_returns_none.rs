mod common;

use common::confirm_state_kit::held;
use rafaello_core::bus::JsonRpcId;
use rafaello_core::gate::ConfirmState;

#[test]
fn try_take_for_timeout_then_try_resolve_returns_none() {
    let state = ConfirmState::new();
    let id = JsonRpcId::from("c-1");
    state.reserve(id.clone(), held());

    assert!(state.try_take_for_timeout(&id).is_some());
    assert!(
        state.try_resolve(&id).is_none(),
        "TimedOut → None on answer path"
    );
}
