mod common;
use common::EnvGuard;
use rafaello_fetch::{reset_taint_override_for_test, take_taint_override};
use serde_json::json;

#[test]
fn taint_override_applies_once_then_clears() {
    reset_taint_override_for_test();
    let raw = json!([{"source": "fixture://once", "detail": null}]).to_string();
    let _g = EnvGuard::set("RFL_FETCH_TEST_TAINT_OVERRIDE", &raw);

    let first = take_taint_override();
    assert!(first.is_some(), "first call must yield the override");

    let second = take_taint_override();
    assert!(
        second.is_none(),
        "second call must yield None (once-per-process)"
    );

    reset_taint_override_for_test();
}
