mod common;

use std::sync::Arc;
use std::thread;

use common::confirm_state_kit::held;
use rafaello_core::bus::JsonRpcId;
use rafaello_core::gate::ConfirmState;

#[test]
fn concurrent_try_resolve_and_try_take_for_timeout_exactly_one_winner() {
    for i in 0..100 {
        let state = Arc::new(ConfirmState::new());
        let id = JsonRpcId::from(format!("c-{i}"));
        state.reserve(id.clone(), held());

        let s_a = Arc::clone(&state);
        let id_a = id.clone();
        let t_resolve = thread::spawn(move || s_a.try_resolve(&id_a).is_some());

        let s_b = Arc::clone(&state);
        let id_b = id.clone();
        let t_timeout = thread::spawn(move || s_b.try_take_for_timeout(&id_b).is_some());

        let r = t_resolve.join().unwrap();
        let t = t_timeout.join().unwrap();
        assert!(
            r ^ t,
            "exactly one winner per confirm_id (iter {i}): resolve={r}, timeout={t}"
        );
    }
}
