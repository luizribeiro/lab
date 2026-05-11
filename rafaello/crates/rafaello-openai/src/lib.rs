//! rafaello-openai: OpenAI Chat Completions wire client.
//!
//! Scope §OP1 / §OP1a. The wire client + error mapping is a
//! self-contained module exercisable against an in-process HTTP
//! stub; the bus integration lands in c33.

pub mod error;
pub mod wire;

pub use error::{
    map_to_assistant, read_required_api_key, read_required_endpoint_url, read_required_model,
    OpenaiConfigError, OpenaiError,
};
pub use wire::{
    translate, ChatCompletionRequest, ChatCompletionResponse, Choice, Msg, ToolCall, ToolCallFn,
    TurnEvent, WireClient,
};
