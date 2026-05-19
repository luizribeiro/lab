use std::io::{self, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::AgentKind;
use crate::app::Term;
use crate::ui::{self, markdown::MarkdownSkin};
use crate::utils::transcripts_dir;

pub struct Transcript {
    path: PathBuf,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum Entry {
    User { content: String },
    Assistant { content: String },
    Tool { name: String, ok: bool },
}

impl Transcript {
    pub fn for_session(agent: AgentKind, id: Uuid) -> Self {
        let dir = transcripts_dir();
        let _ = std::fs::create_dir_all(&dir);
        Self {
            path: dir.join(format!("{}-{}.jsonl", agent.label(), id)),
        }
    }

    pub fn append_turn(&self, user: &str, entries: &[Entry]) -> io::Result<()> {
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
        for entry in entries {
            writeln!(
                f,
                "{}",
                serde_json::to_string(entry).map_err(io::Error::other)?
            )?;
        }
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
            .filter(|e| matches!(e, Entry::User { .. }))
            .count();
        let label = if turns == 1 { "turn" } else { "turns" };
        ui::commit_dim_line(
            terminal,
            &format!("── conversation so far ({turns} {label}) ──"),
        )?;
        let mut last_tool = false;
        for entry in entries {
            match entry {
                Entry::User { content } => {
                    ui::commit_user_prompt(terminal, &content)?;
                    last_tool = false;
                }
                Entry::Assistant { content } => {
                    if last_tool {
                        ui::commit_blank_line(terminal)?;
                    }
                    ui::commit_markdown(terminal, skin, &content)?;
                    last_tool = false;
                }
                Entry::Tool { name, ok } => {
                    ui::commit_tool_result(terminal, &name, ok)?;
                    last_tool = true;
                }
            }
        }
        ui::commit_dim_line(terminal, "── end of history ──")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_round_trips_tool_entries() {
        let dir = tempfile::tempdir().unwrap();
        let transcript = Transcript {
            path: dir.path().join("session.jsonl"),
        };
        let entries = vec![
            Entry::Assistant {
                content: "checking".to_string(),
            },
            Entry::Tool {
                name: "command_execution".to_string(),
                ok: true,
            },
            Entry::Assistant {
                content: "done".to_string(),
            },
        ];

        transcript.append_turn("inspect", &entries).unwrap();

        assert_eq!(
            transcript.load(),
            vec![
                Entry::User {
                    content: "inspect".to_string(),
                },
                Entry::Assistant {
                    content: "checking".to_string(),
                },
                Entry::Tool {
                    name: "command_execution".to_string(),
                    ok: true,
                },
                Entry::Assistant {
                    content: "done".to_string(),
                },
            ]
        );
    }
}
