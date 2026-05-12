//! §TUI-MA1: `TestConfirmAnswerQueue::next_answer` dequeues FIFO.

use rafaello_tui::env::TestConfirmAnswer;
use rafaello_tui::test_confirm_queue::TestConfirmAnswerQueue;

#[test]
fn dequeues_in_fifo_order() {
    let (fatal_tx, _fatal_rx) = tokio::sync::oneshot::channel::<String>();
    let q = TestConfirmAnswerQueue::new(
        vec![
            TestConfirmAnswer::Allow,
            TestConfirmAnswer::Deny,
            TestConfirmAnswer::AlwaysAllowSession,
        ],
        fatal_tx,
    );
    assert_eq!(q.next_answer(), TestConfirmAnswer::Allow);
    assert_eq!(q.next_answer(), TestConfirmAnswer::Deny);
    assert_eq!(q.next_answer(), TestConfirmAnswer::AlwaysAllowSession);
    assert!(q.is_empty());
}
