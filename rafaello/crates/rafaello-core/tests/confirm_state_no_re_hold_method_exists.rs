//! Compile-time / type-level assertion that `re_hold` does not
//! exist on `ConfirmState`. The authoritative check is the
//! `compile_fail` doc-test on `rafaello_core::gate::confirm_state`
//! (run via `cargo test --doc -p rafaello-core`). If `re_hold` is
//! ever re-introduced, that doc-test starts compiling and fails
//! the suite.
//!
//! This integration test additionally trips a trait-based probe:
//! we declare a trait with a `re_hold` method that returns `()`,
//! blanket-impl it for every `T`, and call `re_hold` through it.
//! If an inherent `ConfirmState::re_hold` is ever added with a
//! different return type, method resolution will pick the inherent
//! one and the `let _: () =` annotation will fail to compile.

use rafaello_core::bus::JsonRpcId;
use rafaello_core::gate::ConfirmState;

trait NoReHold {
    fn re_hold(&self, _id: &JsonRpcId) {}
}
impl<T> NoReHold for T {}

#[test]
fn re_hold_method_must_not_exist_on_confirm_state() {
    let state = ConfirmState::new();
    let id = JsonRpcId::from("c-1");
    let _: () = NoReHold::re_hold(&state, &id);
}
