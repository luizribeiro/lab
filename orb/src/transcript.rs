use std::io::{self, Write};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::agent::AgentKind;
use crate::utils::transcripts_dir;

pub struct Transcript {
    path: PathBuf,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum TranscriptEntry {
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

    pub fn append_turn(&self, user: &str, entries: &[TranscriptEntry]) -> io::Result<()> {
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(
            f,
            "{}",
            serde_json::to_string(&TranscriptEntry::User {
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

    pub fn load(&self) -> Vec<TranscriptEntry> {
        let Ok(content) = std::fs::read_to_string(&self.path) else {
            return Vec::new();
        };
        content
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
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
            TranscriptEntry::Assistant {
                content: "checking".to_string(),
            },
            TranscriptEntry::Tool {
                name: "command_execution".to_string(),
                ok: true,
            },
            TranscriptEntry::Assistant {
                content: "done".to_string(),
            },
        ];

        transcript.append_turn("inspect", &entries).unwrap();

        assert_eq!(
            transcript.load(),
            vec![
                TranscriptEntry::User {
                    content: "inspect".to_string(),
                },
                TranscriptEntry::Assistant {
                    content: "checking".to_string(),
                },
                TranscriptEntry::Tool {
                    name: "command_execution".to_string(),
                    ok: true,
                },
                TranscriptEntry::Assistant {
                    content: "done".to_string(),
                },
            ]
        );
    }
}
