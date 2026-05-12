//! §TUI-MA1: scripted multi-answer queue backing `RFL_TUI_TEST_CONFIRM_ANSWERS`.

use std::collections::VecDeque;
use std::sync::Mutex;

use tokio::sync::oneshot;
use tracing::error;

use crate::env::TestConfirmAnswer;

pub struct TestConfirmAnswerQueue {
    inner: Mutex<QueueState>,
}

struct QueueState {
    queue: VecDeque<TestConfirmAnswer>,
    modal_counter: u64,
    fatal_tx: Option<oneshot::Sender<String>>,
}

impl TestConfirmAnswerQueue {
    pub fn new(answers: Vec<TestConfirmAnswer>, fatal_tx: oneshot::Sender<String>) -> Self {
        Self {
            inner: Mutex::new(QueueState {
                queue: VecDeque::from(answers),
                modal_counter: 0,
                fatal_tx: Some(fatal_tx),
            }),
        }
    }

    pub fn next_answer(&self) -> TestConfirmAnswer {
        let mut state = self.inner.lock().expect("queue mutex poisoned");
        state.modal_counter += 1;
        let n = state.modal_counter;
        if let Some(ans) = state.queue.pop_front() {
            return ans;
        }
        let msg = exhaustion_message(n);
        error!("{msg}");
        if let Some(tx) = state.fatal_tx.take() {
            let _ = tx.send(msg.clone());
        }
        drop(state);
        panic!("{msg}");
    }

    pub fn is_empty(&self) -> bool {
        self.inner
            .lock()
            .expect("queue mutex poisoned")
            .queue
            .is_empty()
    }
}

fn exhaustion_message(modal_n: u64) -> String {
    format!("TestConfirmAnswers queue exhausted; modal #{modal_n} had no scripted answer")
}
