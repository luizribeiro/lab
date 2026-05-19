use std::io;

use pilot::{Event as PilotEvent, TurnItem, TurnStream};

use crate::app::Term;
use crate::transcript::Entry as TranscriptEntry;
use crate::ui::{self, markdown::MarkdownSkin};

/// A turn that is currently streaming. Owns the stream plus enough state
/// to render the in-flight UI (pending tool list, buffered assistant text).
pub struct ActiveTurn {
    pub stream: TurnStream,
    pub prompt: String,
    pub text_buffer: String,
    pub pending_text: String,
    pub last_rendered_tool_result: bool,
    pub transcript_entries: Vec<TranscriptEntry>,
    pub pending_tools: Vec<PendingTool>,
}

#[derive(Clone)]
pub struct PendingTool {
    pub call_id: String,
    pub name: String,
}

impl ActiveTurn {
    pub fn new(stream: TurnStream, prompt: String) -> Self {
        Self {
            stream,
            prompt,
            text_buffer: String::new(),
            pending_text: String::new(),
            last_rendered_tool_result: false,
            transcript_entries: Vec::new(),
            pending_tools: Vec::new(),
        }
    }
}

/// Poll the active turn's stream, parking forever if no turn is in flight.
/// This is the right-hand side of the main `tokio::select!`.
pub async fn poll(active: &mut Option<ActiveTurn>) -> Option<Result<TurnItem, pilot::Error>> {
    match active {
        Some(a) => Some(futures_util::StreamExt::next(&mut a.stream).await?),
        None => std::future::pending().await,
    }
}

/// Apply one pilot::Event to the active turn's state. Tool results are
/// committed to scrollback (append-only); pending tool lines live inside
/// the inline viewport and get drawn each frame from `active.pending_tools`.
pub fn process_event(
    active: &mut ActiveTurn,
    ev: PilotEvent,
    terminal: &mut Term,
    skin: &MarkdownSkin,
) -> io::Result<()> {
    match ev {
        PilotEvent::AssistantText { delta } => {
            active.text_buffer.push_str(&delta);
            active.pending_text.push_str(&delta);
        }
        PilotEvent::ToolCall { call_id, name, .. } => {
            flush_pending_text(active, terminal, skin)?;
            active.pending_tools.push(PendingTool { call_id, name });
        }
        PilotEvent::ToolResult { call_id, ok, .. } => {
            flush_pending_text(active, terminal, skin)?;
            if let Some(pos) = active
                .pending_tools
                .iter()
                .position(|t| t.call_id == call_id)
            {
                let tool = active.pending_tools.remove(pos);
                ui::commit_tool_result(terminal, &tool.name, ok)?;
                active.transcript_entries.push(TranscriptEntry::Tool {
                    name: tool.name,
                    ok,
                });
                active.last_rendered_tool_result = true;
            }
        }
        PilotEvent::TurnComplete { ok: false } => {
            flush_pending_text(active, terminal, skin)?;
            ui::commit_status_line(terminal, "(turn reported failure)", ui::CommitColor::Err)?;
        }
        _ => {}
    }
    Ok(())
}

pub fn flush_pending_text(
    active: &mut ActiveTurn,
    terminal: &mut Term,
    skin: &MarkdownSkin,
) -> io::Result<()> {
    let text = active.pending_text.trim();
    if !text.is_empty() {
        if active.last_rendered_tool_result {
            ui::commit_blank_line(terminal)?;
        }
        ui::commit_markdown(terminal, skin, text)?;
        active.transcript_entries.push(TranscriptEntry::Assistant {
            content: text.to_string(),
        });
        active.last_rendered_tool_result = false;
    }
    active.pending_text.clear();
    Ok(())
}
