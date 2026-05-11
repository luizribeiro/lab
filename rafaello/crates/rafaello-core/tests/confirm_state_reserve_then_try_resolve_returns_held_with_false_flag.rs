mod common;

use common::confirm_state_kit::held;
use rafaello_core::bus::JsonRpcId;
use rafaello_core::gate::ConfirmState;

#[test]
fn reserve_then_try_resolve_returns_held_with_false_flag() {
    let state = ConfirmState::new();
    let id = JsonRpcId::from("c-1");
    state.reserve(id.clone(), held());

    let resolved = state.try_resolve(&id).expect("Active → Some");
    assert!(!resolved.1, "session_grant_requested defaults to false");
    assert_eq!(resolved.0.dispatch_target.name(), "tool");
}
