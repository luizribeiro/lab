use std::io::{self, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app::{AgentKind, Term};
use crate::markdown::MarkdownSkin;
use crate::ui;

pub struct Transcript {
    path: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Entry {
    User { content: String },
    Assistant { content: String },
}

impl Transcript {
    pub fn for_session(agent: AgentKind, id: Uuid) -> Self {
        let dir = transcripts_dir();
        let _ = std::fs::create_dir_all(&dir);
        Self {
            path: dir.join(format!("{}-{}.jsonl", agent.label(), id)),
        }
    }

    pub fn append_turn(&self, user: &str, assistant: &str) -> io::Result<()> {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(
            f,
            "{}",
            serde_json::to_string(&Entry::User {
                content: user.to_string()
            })
            .map_err(io::Error::other)?
        )?;
        writeln!(
            f,
            "{}",
            serde_json::to_string(&Entry::Assistant {
                content: assistant.to_string()
            })
            .map_err(io::Error::other)?
        )?;
        Ok(())
    }

    pub fn load(&self) -> Vec<Entry> {
        let Ok(content) = std::fs::read_to_string(&self.path) else {
            return Vec::new();
        };
        content
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    pub fn replay(&self, terminal: &mut Term, skin: &MarkdownSkin) -> io::Result<()> {
        let entries = self.load();
        if entries.is_empty() {
            return Ok(());
        }
        let turns = entries
            .iter()
            .filter(|e| matches!(e, Entry::Assistant { .. }))
            .count();
        let label = if turns == 1 { "turn" } else { "turns" };
        ui::commit_dim_line(
            terminal,
            &format!("── conversation so far ({turns} {label}) ──"),
        )?;
        for entry in entries {
            match entry {
                Entry::User { content } => ui::commit_user_prompt(terminal, &content)?,
                Entry::Assistant { content } => ui::commit_markdown(terminal, skin, &content)?,
            }
        }
        ui::commit_dim_line(terminal, "── end of history ──")?;
        Ok(())
    }
}

fn transcripts_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    home.join(".pilot").join("transcripts")
}
