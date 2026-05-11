//! Scope §OP1: multiple `choices` → use `choices[0]`, stderr warning.

mod common;

use rafaello_openai::{translate, ChatCompletionResponse, Choice, Msg, TurnEvent};

fn mk_choice(idx: u32, text: &str) -> Choice {
    Choice {
        index: idx,
        message: Msg {
            role: "assistant".to_string(),
            content: Some(text.to_string()),
            tool_calls: None,
            tool_call_id: None,
        },
        finish_reason: "stop".to_string(),
    }
}

#[tracing_test::traced_test]
#[test]
fn multiple_choices_uses_first_and_logs_warning() {
    let resp = ChatCompletionResponse {
        id: "cmpl-1".to_string(),
        choices: vec![
            mk_choice(0, "first"),
            mk_choice(1, "second"),
            mk_choice(2, "third"),
        ],
        usage: None,
    };
    let events = translate(resp, &[]);
    assert_eq!(
        events,
        vec![TurnEvent::AssistantMessage("first".to_string())],
        "only choices[0] should be surfaced"
    );
    assert!(
        logs_contain("multiple choices"),
        "expected multi-choices warning in logs"
    );
}
