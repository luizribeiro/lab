//! Public per-conversation handle. Mints a UUID, owns a driver and a
//! workdir, exposes `send` to start a turn.

use std::path::PathBuf;
use std::sync::Arc;

use uuid::Uuid;

use crate::Result;
use crate::driver::{Driver, TurnOptions};
use crate::process::spawn_jsonl;
use crate::turn::{Turn, TurnStream};

/// One conversation with an agent CLI, identified by a UUID that the
/// underlying CLI uses to persist its own state. Spawning a fresh child per
/// turn (`send`) is the uniform model across all drivers; the CLI's session
/// storage handles continuity between turns.
pub struct Session {
    id: Uuid,
    driver: Arc<dyn Driver>,
    workdir: PathBuf,
    recorded_turns: Vec<Turn>,
}

impl Session {
    /// Open a fresh session with a newly-minted UUID.
    pub fn new(driver: Arc<dyn Driver>, workdir: impl Into<PathBuf>) -> Self {
        Self {
            id: Uuid::new_v4(),
            driver,
            workdir: workdir.into(),
            recorded_turns: Vec::new(),
        }
    }

    /// This session's UUID, the key under which the underlying CLI persists
    /// conversation state on disk.
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Working directory passed as `cwd` to each child process.
    pub fn workdir(&self) -> &std::path::Path {
        &self.workdir
    }

    /// Send a prompt and return a stream over the resulting events.
    ///
    /// Spawns a fresh child process per call. The returned `TurnStream`
    /// yields each [`crate::Event`] as it arrives, then exactly one
    /// `TurnItem::Complete(Turn)` when the child exits, then `None`.
    /// Dropping the stream kills the child.
    pub async fn send(&mut self, prompt: &str, opts: TurnOptions) -> Result<TurnStream> {
        let spec = self.driver.command(self.id, prompt, &opts);
        let (handle, rx) = spawn_jsonl(spec, self.workdir.clone()).await?;
        let mut stream = TurnStream::new(handle, rx, self.driver.clone());
        if let Some(d) = opts.timeout {
            stream = stream.with_timeout(d);
        }
        Ok(stream)
    }

    /// Record a completed [`Turn`] in this session's local history. The
    /// CLI itself is the authoritative store (keyed by [`Self::id`]); this
    /// history is observational and exists only because callers find it
    /// convenient.
    pub fn record(&mut self, turn: Turn) {
        self.recorded_turns.push(turn);
    }

    /// Read the locally-recorded turn history.
    pub fn recorded_turns(&self) -> &[Turn] {
        &self.recorded_turns
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::driver::CommandSpec;
    use crate::event::Event;
    use crate::{Error, ParseError};
    use futures_util::StreamExt;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn fake_agent() -> PathBuf {
        let mut p = std::env::current_exe().unwrap();
        p.pop();
        if p.ends_with("deps") {
            p.pop();
        }
        p.push(format!("fake_agent{}", std::env::consts::EXE_SUFFIX));
        p
    }

    struct ScriptDriver {
        script: PathBuf,
    }

    impl Driver for ScriptDriver {
        fn name(&self) -> &'static str {
            "script"
        }
        fn command(&self, _session_id: Uuid, _prompt: &str, _opts: &TurnOptions) -> CommandSpec {
            CommandSpec {
                program: fake_agent(),
                args: vec!["--script".into(), self.script.to_string_lossy().into()],
                env: vec![],
            }
        }
        fn parse(&self, value: serde_json::Value) -> std::result::Result<Vec<Event>, ParseError> {
            Ok(vec![Event::Raw {
                driver: "script",
                value,
            }])
        }
    }

    fn write_script(lines: &[&str]) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        for l in lines {
            writeln!(f, "{l}").unwrap();
        }
        f.flush().unwrap();
        f
    }

    #[tokio::test]
    async fn new_mints_unique_uuids() {
        let s1 = Session::new(
            Arc::new(ScriptDriver {
                script: PathBuf::new(),
            }),
            "/tmp",
        );
        let s2 = Session::new(
            Arc::new(ScriptDriver {
                script: PathBuf::new(),
            }),
            "/tmp",
        );
        assert_ne!(s1.id(), s2.id());
    }

    #[tokio::test]
    async fn send_streams_events_from_driver_command() {
        let script = write_script(&[r#"emit {"n":1}"#, r#"emit {"n":2}"#, "exit 0"]);
        let driver: Arc<dyn Driver> = Arc::new(ScriptDriver {
            script: script.path().to_path_buf(),
        });
        let mut session = Session::new(driver, std::env::temp_dir());

        let mut stream = session
            .send("anything", TurnOptions::default())
            .await
            .expect("send");
        let mut events = 0;
        let mut completed = false;
        while let Some(item) = stream.next().await {
            match item.expect("ok") {
                crate::TurnItem::Event(_) => events += 1,
                crate::TurnItem::Complete(_) => completed = true,
            }
        }
        assert_eq!(events, 2);
        assert!(completed);
    }

    #[tokio::test]
    async fn send_honors_turnoptions_timeout() {
        let script = write_script(&["sleep 30000"]);
        let driver: Arc<dyn Driver> = Arc::new(ScriptDriver {
            script: script.path().to_path_buf(),
        });
        let mut session = Session::new(driver, std::env::temp_dir());

        let opts = TurnOptions {
            timeout: Some(std::time::Duration::from_millis(150)),
            ..Default::default()
        };
        let mut stream = session.send("hi", opts).await.expect("send");
        let start = std::time::Instant::now();
        let item = stream.next().await.expect("first").expect_err("timeout");
        assert!(matches!(item, Error::Timeout(_)));
        assert!(start.elapsed() < std::time::Duration::from_secs(2));
    }

    #[tokio::test]
    async fn record_and_recorded_turns_round_trip() {
        let driver: Arc<dyn Driver> = Arc::new(ScriptDriver {
            script: PathBuf::new(),
        });
        let mut session = Session::new(driver, "/tmp");
        assert!(session.recorded_turns().is_empty());

        session.record(Turn {
            events: vec![Event::AssistantText { delta: "hi".into() }],
        });
        session.record(Turn { events: vec![] });
        assert_eq!(session.recorded_turns().len(), 2);
    }

    #[tokio::test]
    async fn send_uses_session_uuid_as_command_argument() {
        struct UuidCapturingDriver {
            seen: std::sync::Mutex<Option<Uuid>>,
            script: PathBuf,
        }
        impl Driver for UuidCapturingDriver {
            fn name(&self) -> &'static str {
                "uuid-cap"
            }
            fn command(&self, sid: Uuid, _p: &str, _o: &TurnOptions) -> CommandSpec {
                *self.seen.lock().unwrap() = Some(sid);
                CommandSpec {
                    program: fake_agent(),
                    args: vec!["--script".into(), self.script.to_string_lossy().into()],
                    env: vec![],
                }
            }
            fn parse(&self, v: serde_json::Value) -> std::result::Result<Vec<Event>, ParseError> {
                Ok(vec![Event::Raw {
                    driver: "uuid-cap",
                    value: v,
                }])
            }
        }
        let script = write_script(&["exit 0"]);
        let driver = Arc::new(UuidCapturingDriver {
            seen: std::sync::Mutex::new(None),
            script: script.path().to_path_buf(),
        });
        let driver_dyn: Arc<dyn Driver> = driver.clone();
        let mut session = Session::new(driver_dyn, std::env::temp_dir());
        let expected_id = session.id();

        for _ in 0..2 {
            let mut stream = session
                .send("hi", TurnOptions::default())
                .await
                .expect("send");
            while stream.next().await.is_some() {}
        }
        let seen = *driver.seen.lock().unwrap();
        assert_eq!(seen, Some(expected_id));
    }
}
