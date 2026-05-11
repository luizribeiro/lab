//! Scope §OP1: empty `choices` → single assistant_message `"(no response)"`.

mod common;

use rafaello_openai::{translate, ChatCompletionResponse, TurnEvent};

#[test]
fn empty_choices_emits_no_response() {
    let resp = ChatCompletionResponse {
        id: "cmpl-1".to_string(),
        choices: vec![],
        usage: None,
    };
    let events = translate(resp, &[]);
    assert_eq!(
        events,
        vec![TurnEvent::AssistantMessage("(no response)".to_string())]
    );
}
