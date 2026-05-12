//! §TUI-MA1: exhaustion emits a fatal message AND panics — belt-and-suspenders.

use std::panic::{catch_unwind, AssertUnwindSafe};

use rafaello_tui::env::TestConfirmAnswer;
use rafaello_tui::test_confirm_queue::TestConfirmAnswerQueue;

#[test]
fn second_call_with_one_scripted_answer_sends_fatal_then_panics() {
    let (fatal_tx, mut fatal_rx) = tokio::sync::oneshot::channel::<String>();
    let q = TestConfirmAnswerQueue::new(vec![TestConfirmAnswer::Allow], fatal_tx);

    assert_eq!(q.next_answer(), TestConfirmAnswer::Allow);

    let result = catch_unwind(AssertUnwindSafe(|| q.next_answer()));
    assert!(result.is_err(), "second next_answer must panic");

    let payload = result.unwrap_err();
    let panic_msg = if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&'static str>() {
        s.to_string()
    } else {
        panic!("unexpected panic payload");
    };

    let expected = "TestConfirmAnswers queue exhausted; modal #2 had no scripted answer";
    assert_eq!(panic_msg, expected);

    let fatal_msg = fatal_rx
        .try_recv()
        .expect("fatal_tx must have sent before panicking");
    assert_eq!(fatal_msg, expected);
}
