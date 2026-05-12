mod common;
use common::EnvGuard;
use rafaello_fetch::{reset_taint_override_for_test, take_taint_override};

#[tracing_test::traced_test]
#[test]
fn taint_override_malformed_json_falls_back_to_none() {
    reset_taint_override_for_test();
    let _g = EnvGuard::set("RFL_FETCH_TEST_TAINT_OVERRIDE", "not-json{");
    let got = take_taint_override();
    assert!(got.is_none(), "malformed JSON must yield None");
    assert!(
        logs_contain("malformed RFL_FETCH_TEST_TAINT_OVERRIDE JSON"),
        "expected error log"
    );
    reset_taint_override_for_test();
}
