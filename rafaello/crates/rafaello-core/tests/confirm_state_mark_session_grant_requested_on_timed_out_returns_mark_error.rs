mod common;

use common::confirm_state_kit::held;
use rafaello_core::bus::JsonRpcId;
use rafaello_core::gate::{ConfirmState, MarkError};

#[test]
fn mark_session_grant_requested_on_timed_out_returns_mark_error() {
    let state = ConfirmState::new();
    let id = JsonRpcId::from("c-1");
    state.reserve(id.clone(), held());

    assert!(state.try_take_for_timeout(&id).is_some());

    match state.mark_session_grant_requested(&id) {
        Err(MarkError::NotActive) => {}
        other => panic!("expected NotActive, got {other:?}"),
    }
}
