//! Scope §OP1: `tool_calls[i].function.arguments` parse error →
//! assistant_message `"openai: invalid tool args from model: ..."`;
//! do **not** emit the malformed tool_request.

mod common;

use rafaello_openai::{
    translate, ChatCompletionResponse, Choice, Msg, ToolCall, ToolCallFn, TurnEvent,
};

#[test]
fn invalid_tool_arguments_emits_error_assistant_only() {
    let resp = ChatCompletionResponse {
        id: "cmpl-1".to_string(),
        choices: vec![Choice {
            index: 0,
            message: Msg {
                role: "assistant".to_string(),
                content: None,
                tool_calls: Some(vec![ToolCall {
                    id: "call_1".to_string(),
                    kind: "function".to_string(),
                    function: ToolCallFn {
                        name: "readfile".to_string(),
                        arguments: "{not valid json".to_string(),
                    },
                }]),
                tool_call_id: None,
            },
            finish_reason: "tool_calls".to_string(),
        }],
        usage: None,
    };
    let known = vec!["readfile".to_string()];
    let events = translate(resp, &known);
    assert_eq!(
        events.len(),
        1,
        "no tool_request must be emitted, got {events:?}"
    );
    match &events[0] {
        TurnEvent::AssistantMessage(text) => assert!(
            text.starts_with("openai: invalid tool args from model: "),
            "got {text:?}"
        ),
        other => panic!("expected AssistantMessage, got {other:?}"),
    }
}
