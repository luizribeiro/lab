//! §TUI-MA1: `RFL_TUI_TEST_CONFIRM_ANSWERS` parses a comma list into a Vec.

use std::collections::HashMap;

use rafaello_tui::env::{
    load_from, TestConfirmAnswer, RFL_BUS_FD, RFL_PROJECT_ROOT, RFL_TUI_TEST_CONFIRM_ANSWERS,
};

#[test]
fn parses_three_entries_in_order() {
    let map: HashMap<&'static str, &'static str> = HashMap::from([
        (RFL_BUS_FD, "3"),
        (RFL_PROJECT_ROOT, "/abs/path"),
        (RFL_TUI_TEST_CONFIRM_ANSWERS, "allow,deny,timeout"),
    ]);
    let env = load_from(|k| map.get(k).map(|v| v.to_string())).expect("parse");
    assert_eq!(
        env.test_confirm_answers,
        Some(vec![
            TestConfirmAnswer::Allow,
            TestConfirmAnswer::Deny,
            TestConfirmAnswer::Timeout,
        ])
    );
    assert!(env.test_confirm_answer.is_none());
}
