//! Public per-conversation handle. Mints a UUID, owns a driver and a
//! workdir, exposes `send` to start a turn.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use uuid::Uuid;

use crate::Result;
use crate::driver::{Driver, TurnOptions};
use crate::process::spawn_jsonl;
use crate::turn::{Turn, TurnStream};

fn session_lock_for(driver: &str, id: Uuid) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    type Registry = Mutex<HashMap<(String, Uuid), std::sync::Arc<tokio::sync::Mutex<()>>>>;
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    let registry = REGISTRY.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = registry.lock().unwrap_or_else(|e| e.into_inner());
    map.entry((driver.to_string(), id))
        .or_insert_with(|| std::sync::Arc::new(tokio::sync::Mutex::new(())))
        .clone()
}

/// One conversation with an agent CLI, identified by a UUID that the
/// underlying CLI uses to persist its own state. Spawning a fresh child per
/// turn (`send`) is the uniform model across all drivers; the CLI's session
/// storage handles continuity between turns.
pub struct Session {
    id: Uuid,
    driver: Arc<dyn Driver>,
    workdir: PathBuf,
    recorded_turns: Vec<Turn>,
    busy: std::sync::Arc<std::sync::atomic::AtomicBool>,
    turns_completed: Arc<AtomicUsize>,
}

impl Session {
    /// Open a fresh session with a newly-minted UUID.
    pub fn new(driver: Arc<dyn Driver>, workdir: impl Into<PathBuf>) -> Self {
        Self {
            id: Uuid::new_v4(),
            driver,
            workdir: workdir.into(),
            recorded_turns: Vec::new(),
            busy: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            turns_completed: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Rehydrate a session from a previously-persisted UUID. Use this when
    /// your program stored the [`Self::id`] of an earlier session and wants
    /// to continue that conversation — the underlying CLI keeps its own
    /// conversation state on disk keyed by this UUID.
    ///
    /// This is a pure constructor: no process is spawned and no IO occurs.
    pub fn resume(driver: Arc<dyn Driver>, id: Uuid, workdir: impl Into<PathBuf>) -> Self {
        Self {
            id,
            driver,
            workdir: workdir.into(),
            recorded_turns: Vec::new(),
            busy: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            turns_completed: Arc::new(AtomicUsize::new(1)),
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
        if self
            .busy
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(crate::Error::Busy);
        }
        let guard = crate::turn::BusyGuard {
            flag: self.busy.clone(),
        };
        let session_lock = session_lock_for(self.driver.name(), self.id);
        let owned_guard = match session_lock.try_lock_owned() {
            Ok(g) => g,
            Err(_) => {
                self.busy.store(false, Ordering::SeqCst);
                return Err(crate::Error::Busy);
            }
        };
        let session_guard = crate::turn::SessionGuard {
            _owned_lock: owned_guard,
        };
        let spec = match if self.turns_completed.load(Ordering::SeqCst) == 0 {
            self.driver.command(self.id, prompt, &opts)
        } else {
            self.driver.resume_command(self.id, prompt, &opts)
        } {
            Ok(s) => s,
            Err(e) => {
                self.busy.store(false, Ordering::SeqCst);
                return Err(e);
            }
        };
        let (handle, rx) = spawn_jsonl(spec, self.workdir.clone()).await?;
        let stream = TurnStream::new(self.id, handle, rx, self.driver.clone())
            .with_completion_counter(self.turns_completed.clone())
            .with_busy_guard(guard)
            .with_session_guard(session_guard);
        let stream = if let Some(d) = opts.timeout {
            stream.with_timeout(d)
        } else {
            stream
        };
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
        fn command(
            &self,
            _session_id: Uuid,
            _prompt: &str,
            _opts: &TurnOptions,
        ) -> crate::Result<CommandSpec> {
            Ok(CommandSpec {
                program: fake_agent(),
                args: vec!["--script".into(), self.script.to_string_lossy().into()],
                env: vec![],
            })
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
            fn command(&self, sid: Uuid, _p: &str, _o: &TurnOptions) -> crate::Result<CommandSpec> {
                *self.seen.lock().unwrap() = Some(sid);
                Ok(CommandSpec {
                    program: fake_agent(),
                    args: vec!["--script".into(), self.script.to_string_lossy().into()],
                    env: vec![],
                })
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

    #[tokio::test]
    async fn second_send_while_first_in_flight_is_rejected() {
        let script = write_script(&["sleep 30000"]);
        let driver: Arc<dyn Driver> = Arc::new(ScriptDriver {
            script: script.path().to_path_buf(),
        });
        let mut session = Session::new(driver, std::env::temp_dir());

        let _first = session
            .send("hi", TurnOptions::default())
            .await
            .expect("first send");
        let err = match session.send("hi again", TurnOptions::default()).await {
            Ok(_) => panic!("second send must be rejected"),
            Err(e) => e,
        };
        assert!(matches!(err, Error::Busy));
    }

    #[tokio::test]
    async fn second_send_works_after_first_completes() {
        let script = write_script(&[r#"emit {"n":1}"#, "exit 0"]);
        let driver: Arc<dyn Driver> = Arc::new(ScriptDriver {
            script: script.path().to_path_buf(),
        });
        let mut session = Session::new(driver, std::env::temp_dir());

        let mut s1 = session
            .send("first", TurnOptions::default())
            .await
            .expect("send 1");
        while s1.next().await.is_some() {}
        let _s2 = session
            .send("second", TurnOptions::default())
            .await
            .expect("send 2 should succeed after first completes");
    }

    #[tokio::test]
    async fn second_send_works_after_first_dropped() {
        let script = write_script(&["sleep 30000"]);
        let driver: Arc<dyn Driver> = Arc::new(ScriptDriver {
            script: script.path().to_path_buf(),
        });
        let mut session = Session::new(driver, std::env::temp_dir());

        let s1 = session
            .send("first", TurnOptions::default())
            .await
            .expect("send 1");
        drop(s1);
        let _s2 = session
            .send("second", TurnOptions::default())
            .await
            .expect("send 2 should succeed after first dropped");
    }

    #[tokio::test]
    async fn resume_preserves_supplied_uuid() {
        let id = Uuid::new_v4();
        let driver: Arc<dyn Driver> = Arc::new(ScriptDriver {
            script: PathBuf::new(),
        });
        let session = Session::resume(driver, id, "/tmp");
        assert_eq!(session.id(), id);
    }

    #[tokio::test]
    async fn resume_does_not_spawn_a_process() {
        struct NeverSpawnDriver;
        impl Driver for NeverSpawnDriver {
            fn name(&self) -> &'static str {
                "never-spawn"
            }
            fn command(&self, _: Uuid, _: &str, _: &TurnOptions) -> crate::Result<CommandSpec> {
                panic!("command() must not be called by resume()");
            }
            fn parse(&self, v: serde_json::Value) -> std::result::Result<Vec<Event>, ParseError> {
                Ok(vec![Event::Raw {
                    driver: "never-spawn",
                    value: v,
                }])
            }
        }
        let id = Uuid::new_v4();
        let _session = Session::resume(Arc::new(NeverSpawnDriver), id, "/tmp");
    }

    #[tokio::test]
    async fn first_send_uses_command_subsequent_uses_resume_command() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct CountingDriver {
            command_calls: AtomicUsize,
            resume_calls: AtomicUsize,
            script: PathBuf,
        }
        impl Driver for CountingDriver {
            fn name(&self) -> &'static str {
                "counting"
            }
            fn command(&self, _: Uuid, _: &str, _: &TurnOptions) -> crate::Result<CommandSpec> {
                self.command_calls.fetch_add(1, Ordering::SeqCst);
                Ok(CommandSpec {
                    program: fake_agent(),
                    args: vec!["--script".into(), self.script.to_string_lossy().into()],
                    env: vec![],
                })
            }
            fn resume_command(
                &self,
                _: Uuid,
                _: &str,
                _: &TurnOptions,
            ) -> crate::Result<CommandSpec> {
                self.resume_calls.fetch_add(1, Ordering::SeqCst);
                Ok(CommandSpec {
                    program: fake_agent(),
                    args: vec!["--script".into(), self.script.to_string_lossy().into()],
                    env: vec![],
                })
            }
            fn parse(&self, v: serde_json::Value) -> std::result::Result<Vec<Event>, ParseError> {
                Ok(vec![Event::Raw {
                    driver: "counting",
                    value: v,
                }])
            }
        }

        let script = write_script(&["exit 0"]);
        let driver = Arc::new(CountingDriver {
            command_calls: AtomicUsize::new(0),
            resume_calls: AtomicUsize::new(0),
            script: script.path().to_path_buf(),
        });
        let driver_dyn: Arc<dyn Driver> = driver.clone();
        let mut session = Session::new(driver_dyn, std::env::temp_dir());

        let mut s = session
            .send("turn1", TurnOptions::default())
            .await
            .expect("send 1");
        while s.next().await.is_some() {}
        let mut s = session
            .send("turn2", TurnOptions::default())
            .await
            .expect("send 2");
        while s.next().await.is_some() {}
        let mut s = session
            .send("turn3", TurnOptions::default())
            .await
            .expect("send 3");
        while s.next().await.is_some() {}

        assert_eq!(
            driver.command_calls.load(Ordering::SeqCst),
            1,
            "command only on first turn"
        );
        assert_eq!(
            driver.resume_calls.load(Ordering::SeqCst),
            2,
            "resume on turns 2 and 3"
        );
    }

    #[tokio::test]
    async fn failed_first_turn_still_uses_command_on_retry() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct CountingDriver {
            command_calls: AtomicUsize,
            resume_calls: AtomicUsize,
            script: PathBuf,
        }
        impl Driver for CountingDriver {
            fn name(&self) -> &'static str {
                "counting"
            }
            fn command(&self, _: Uuid, _: &str, _: &TurnOptions) -> crate::Result<CommandSpec> {
                self.command_calls.fetch_add(1, Ordering::SeqCst);
                Ok(CommandSpec {
                    program: fake_agent(),
                    args: vec!["--script".into(), self.script.to_string_lossy().into()],
                    env: vec![],
                })
            }
            fn resume_command(
                &self,
                _: Uuid,
                _: &str,
                _: &TurnOptions,
            ) -> crate::Result<CommandSpec> {
                self.resume_calls.fetch_add(1, Ordering::SeqCst);
                Ok(CommandSpec {
                    program: fake_agent(),
                    args: vec!["--script".into(), self.script.to_string_lossy().into()],
                    env: vec![],
                })
            }
            fn parse(&self, v: serde_json::Value) -> std::result::Result<Vec<Event>, ParseError> {
                Ok(vec![Event::Raw {
                    driver: "counting",
                    value: v,
                }])
            }
        }

        let script = write_script(&["exit 1"]);
        let driver = Arc::new(CountingDriver {
            command_calls: AtomicUsize::new(0),
            resume_calls: AtomicUsize::new(0),
            script: script.path().to_path_buf(),
        });
        let driver_dyn: Arc<dyn Driver> = driver.clone();
        let mut session = Session::new(driver_dyn, std::env::temp_dir());

        let mut s1 = session
            .send("turn1", TurnOptions::default())
            .await
            .expect("send 1");
        while s1.next().await.is_some() {}
        drop(s1);

        let mut s2 = session
            .send("turn2", TurnOptions::default())
            .await
            .expect("send 2");
        while s2.next().await.is_some() {}

        assert_eq!(
            driver.command_calls.load(Ordering::SeqCst),
            2,
            "both turns use command on retry"
        );
        assert_eq!(driver.resume_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn resumed_session_uses_resume_command_on_first_send() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct CountingDriver {
            command_calls: AtomicUsize,
            resume_calls: AtomicUsize,
            script: PathBuf,
        }
        impl Driver for CountingDriver {
            fn name(&self) -> &'static str {
                "counting"
            }
            fn command(&self, _: Uuid, _: &str, _: &TurnOptions) -> crate::Result<CommandSpec> {
                self.command_calls.fetch_add(1, Ordering::SeqCst);
                Ok(CommandSpec {
                    program: fake_agent(),
                    args: vec!["--script".into(), self.script.to_string_lossy().into()],
                    env: vec![],
                })
            }
            fn resume_command(
                &self,
                _: Uuid,
                _: &str,
                _: &TurnOptions,
            ) -> crate::Result<CommandSpec> {
                self.resume_calls.fetch_add(1, Ordering::SeqCst);
                Ok(CommandSpec {
                    program: fake_agent(),
                    args: vec!["--script".into(), self.script.to_string_lossy().into()],
                    env: vec![],
                })
            }
            fn parse(&self, v: serde_json::Value) -> std::result::Result<Vec<Event>, ParseError> {
                Ok(vec![Event::Raw {
                    driver: "counting",
                    value: v,
                }])
            }
        }
        let script = write_script(&["exit 0"]);
        let driver = Arc::new(CountingDriver {
            command_calls: AtomicUsize::new(0),
            resume_calls: AtomicUsize::new(0),
            script: script.path().to_path_buf(),
        });
        let driver_dyn: Arc<dyn Driver> = driver.clone();
        let mut session = Session::resume(driver_dyn, Uuid::new_v4(), std::env::temp_dir());
        let mut s = session
            .send("continuation", TurnOptions::default())
            .await
            .expect("send");
        while s.next().await.is_some() {}

        assert_eq!(driver.command_calls.load(Ordering::SeqCst), 0);
        assert_eq!(driver.resume_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn resumed_session_uses_supplied_uuid_in_send() {
        struct UuidCapturingDriver {
            seen: std::sync::Mutex<Option<Uuid>>,
            script: PathBuf,
        }
        impl Driver for UuidCapturingDriver {
            fn name(&self) -> &'static str {
                "uuid-cap"
            }
            fn command(&self, sid: Uuid, _p: &str, _o: &TurnOptions) -> crate::Result<CommandSpec> {
                *self.seen.lock().unwrap() = Some(sid);
                Ok(CommandSpec {
                    program: fake_agent(),
                    args: vec!["--script".into(), self.script.to_string_lossy().into()],
                    env: vec![],
                })
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
        let id = Uuid::new_v4();
        let mut session = Session::resume(driver_dyn, id, std::env::temp_dir());

        let mut stream = session
            .send("anything", TurnOptions::default())
            .await
            .expect("send");
        while stream.next().await.is_some() {}

        assert_eq!(*driver.seen.lock().unwrap(), Some(id));
    }

    #[tokio::test]
    async fn cross_session_lock_blocks_concurrent_same_uuid() {
        let script = write_script(&["sleep 30000"]);
        let driver: Arc<dyn Driver> = Arc::new(ScriptDriver {
            script: script.path().to_path_buf(),
        });
        let id = Uuid::new_v4();
        let mut a = Session::resume(driver.clone(), id, std::env::temp_dir());
        let mut b = Session::resume(driver, id, std::env::temp_dir());

        let _stream_a = a
            .send("first", TurnOptions::default())
            .await
            .expect("a send");
        let err = b
            .send("conflict", TurnOptions::default())
            .await
            .expect_err("b must fail");
        assert!(matches!(err, Error::Busy));
    }

    #[tokio::test]
    async fn cross_session_lock_releases_after_first_session_drops_stream() {
        let script_a = write_script(&[r#"emit {"n":1}"#, "exit 0"]);
        let script_b = write_script(&["exit 0"]);
        let driver_a: Arc<dyn Driver> = Arc::new(ScriptDriver {
            script: script_a.path().to_path_buf(),
        });
        let driver_b: Arc<dyn Driver> = Arc::new(ScriptDriver {
            script: script_b.path().to_path_buf(),
        });
        let id = Uuid::new_v4();
        let mut a = Session::resume(driver_a, id, std::env::temp_dir());
        let mut b = Session::resume(driver_b, id, std::env::temp_dir());

        let mut s_a = a
            .send("a-turn", TurnOptions::default())
            .await
            .expect("a send");
        while s_a.next().await.is_some() {}
        drop(s_a);

        let _s_b = b
            .send("b-turn", TurnOptions::default())
            .await
            .expect("b send should now succeed");
    }
}
