//! Scope §OP1: model proposes a tool name not in `core.tools_list`
//! → assistant_message `"openai: model proposed unknown tool '<name>'"`;
//! do not emit the tool_request.

mod common;

use rafaello_openai::{
    translate, ChatCompletionResponse, Choice, Msg, ToolCall, ToolCallFn, TurnEvent,
};

#[test]
fn unknown_tool_name_emits_error_assistant() {
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
                        name: "delete_universe".to_string(),
                        arguments: "{}".to_string(),
                    },
                }]),
                tool_call_id: None,
            },
            finish_reason: "tool_calls".to_string(),
        }],
        usage: None,
    };
    let known = vec!["readfile".to_string(), "mailcat.send".to_string()];
    let events = translate(resp, &known);
    assert_eq!(
        events,
        vec![TurnEvent::AssistantMessage(
            "openai: model proposed unknown tool 'delete_universe'".to_string()
        )]
    );
}
