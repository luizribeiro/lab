pub mod components;
pub mod live_viewport;
pub mod markdown;
pub mod scrollback;
pub mod terminal;

pub use live_viewport::draw;
pub use scrollback::{
    StatusColor, append_blank_line, append_header, append_markdown, append_status_line,
    append_tool_result, append_user_prompt, replay_transcript,
};
