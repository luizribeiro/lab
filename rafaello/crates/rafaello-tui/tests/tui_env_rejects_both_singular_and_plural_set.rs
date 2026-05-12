//! §TUI-MA1 / pi-4 N-3: setting both singular and plural confirm-answer envs is
//! a startup error with a pinned, snapshot-tested message.

use std::collections::HashMap;

use rafaello_tui::env::{
    load_from, RFL_BUS_FD, RFL_PROJECT_ROOT, RFL_TUI_TEST_CONFIRM_ANSWER,
    RFL_TUI_TEST_CONFIRM_ANSWERS,
};

#[test]
fn mutual_exclusion_error_string_is_pinned() {
    let map: HashMap<&'static str, &'static str> = HashMap::from([
        (RFL_BUS_FD, "3"),
        (RFL_PROJECT_ROOT, "/abs/path"),
        (RFL_TUI_TEST_CONFIRM_ANSWER, "allow"),
        (RFL_TUI_TEST_CONFIRM_ANSWERS, "allow,deny"),
    ]);
    let err =
        load_from(|k| map.get(k).map(|v| v.to_string())).expect_err("mutual exclusion must reject");
    assert_eq!(
        err.to_string(),
        "RFL_TUI_TEST_CONFIRM_ANSWER and RFL_TUI_TEST_CONFIRM_ANSWERS are mutually exclusive; \
         set one or the other"
    );
}
