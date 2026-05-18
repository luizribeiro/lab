//! Test utilities for downstream crates. Gated behind the `test-support`
//! Cargo feature.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use uuid::Uuid;

use crate::driver::{CommandSpec, Driver, TurnInput, TurnOptions};
use crate::{Event, ParseError};

pub struct TestDriver {
    pub name: &'static str,
    pub program: PathBuf,
}

impl TestDriver {
    pub fn new(name: &'static str, program: impl Into<PathBuf>) -> Self {
        Self {
            name,
            program: program.into(),
        }
    }
}

impl Driver for TestDriver {
    fn name(&self) -> &'static str {
        self.name
    }

    fn command(
        &self,
        session_id: Uuid,
        input: &TurnInput,
        _opts: &TurnOptions,
    ) -> crate::Result<CommandSpec> {
        #[allow(unreachable_patterns)]
        let prompt = match input {
            TurnInput::Text(s) => s.as_str(),
            _ => {
                return Err(crate::Error::UnsupportedOption {
                    driver: self.name,
                    option: "non-text TurnInput",
                });
            }
        };
        Ok(CommandSpec {
            program: self.program.clone(),
            args: vec![
                "--session".into(),
                session_id.to_string(),
                "--prompt".into(),
                prompt.into(),
            ],
            env: Vec::new(),
        })
    }

    fn parse(&self, value: serde_json::Value) -> std::result::Result<Vec<Event>, ParseError> {
        Ok(vec![Event::Raw {
            driver: self.name,
            value,
        }])
    }
}

/// A `Driver` wrapper that tees every raw JSON value to a file before
/// forwarding to the inner driver's `parse`.
pub struct RecordingDriver<D: Driver> {
    inner: D,
    file: Mutex<File>,
    recording_failed: Arc<AtomicBool>,
}

impl<D: Driver> RecordingDriver<D> {
    pub fn new(inner: D, path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let file = File::create(&path)?;
        Ok(Self {
            inner,
            file: Mutex::new(file),
            recording_failed: Arc::new(AtomicBool::new(false)),
        })
    }

    /// True if any recording write or lock acquisition failed during this
    /// session's lifetime. Use this in test wrappers to assert recording was
    /// complete before treating the captured fixture as authoritative.
    pub fn recording_failed(&self) -> bool {
        self.recording_failed.load(Ordering::SeqCst)
    }

    /// Returns a shared handle to the failure flag. Clone before passing the
    /// driver into `Session::new`, then check the handle after the session
    /// completes.
    pub fn failure_signal(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.recording_failed)
    }
}

impl<D: Driver> Driver for RecordingDriver<D> {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn command(
        &self,
        session_id: Uuid,
        input: &TurnInput,
        opts: &TurnOptions,
    ) -> crate::Result<CommandSpec> {
        self.inner.command(session_id, input, opts)
    }

    fn resume_command(
        &self,
        session_id: Uuid,
        input: &TurnInput,
        opts: &TurnOptions,
    ) -> crate::Result<CommandSpec> {
        self.inner.resume_command(session_id, input, opts)
    }

    fn observe(&self, session_id: Uuid, raw: &serde_json::Value) {
        self.inner.observe(session_id, raw);
    }

    fn parse(&self, value: serde_json::Value) -> std::result::Result<Vec<Event>, ParseError> {
        match serde_json::to_string(&value) {
            Ok(line) => match self.file.lock() {
                Ok(mut f) => {
                    if let Err(e) = writeln!(f, "{line}") {
                        tracing::warn!(error = %e, "RecordingDriver: file write failed");
                        self.recording_failed.store(true, Ordering::SeqCst);
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, "RecordingDriver: mutex poisoned");
                    self.recording_failed.store(true, Ordering::SeqCst);
                }
            },
            Err(e) => {
                tracing::warn!(error = %e, "RecordingDriver: serialization failed");
                self.recording_failed.store(true, Ordering::SeqCst);
            }
        }
        self.inner.parse(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_carries_session_and_prompt() {
        let d = TestDriver::new("t", "/bin/echo");
        let spec = d
            .command(
                Uuid::nil(),
                &TurnInput::Text("hi".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        assert!(spec.args.iter().any(|a| a == &Uuid::nil().to_string()));
        assert!(spec.args.iter().any(|a| a == "hi"));
    }

    #[test]
    fn parse_returns_raw() {
        let d = TestDriver::new("t", "/bin/echo");
        let evs = d.parse(serde_json::json!({"x": 1})).unwrap();
        assert_eq!(evs.len(), 1);
        assert!(matches!(evs[0], Event::Raw { driver: "t", .. }));
    }

    #[test]
    fn recording_driver_writes_raw_lines() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_path_buf();
        let inner = TestDriver::new("t", "/bin/echo");
        let rec = RecordingDriver::new(inner, &path).unwrap();

        let v1 = serde_json::json!({"type":"a","n":1});
        let v2 = serde_json::json!({"type":"b","n":2});
        let _ = rec.parse(v1.clone()).unwrap();
        let _ = rec.parse(v2.clone()).unwrap();
        drop(rec);

        let content = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 2);
        let parsed1: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        let parsed2: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(parsed1, v1);
        assert_eq!(parsed2, v2);
    }

    #[test]
    fn recording_driver_forwards_to_inner_parse() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let inner = TestDriver::new("inner", "/bin/echo");
        let rec = RecordingDriver::new(inner, tmp.path()).unwrap();
        let v = serde_json::json!({"x": 1});
        let evs = rec.parse(v.clone()).unwrap();
        assert_eq!(evs.len(), 1);
        assert!(matches!(
            &evs[0],
            Event::Raw {
                driver: "inner",
                ..
            }
        ));
    }

    #[test]
    fn recording_driver_forwards_command() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let inner = TestDriver::new("inner", "/bin/echo");
        let rec = RecordingDriver::new(inner, tmp.path()).unwrap();
        let spec = rec
            .command(
                Uuid::nil(),
                &TurnInput::Text("hi".into()),
                &TurnOptions::default(),
            )
            .unwrap();
        assert!(spec.args.contains(&"hi".to_string()));
    }

    #[test]
    fn recording_failed_is_false_after_successful_writes() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let inner = TestDriver::new("inner", "/bin/echo");
        let rec = RecordingDriver::new(inner, tmp.path()).unwrap();
        let _ = rec.parse(serde_json::json!({"x": 1}));
        assert!(!rec.recording_failed());
    }

    #[test]
    fn failure_signal_survives_driver_move() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let inner = TestDriver::new("inner", "/bin/echo");
        let rec = RecordingDriver::new(inner, tmp.path()).unwrap();
        let signal = rec.failure_signal();
        let arc_rec: Arc<dyn Driver> = Arc::new(rec);
        let _ = arc_rec.parse(serde_json::json!({"x": 1}));
        drop(arc_rec);
        assert!(!signal.load(Ordering::SeqCst));
    }

    #[test]
    fn recording_driver_creates_parent_dir() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/sub/recording.jsonl");
        let inner = TestDriver::new("inner", "/bin/echo");
        let rec = RecordingDriver::new(inner, &path).unwrap();
        let _ = rec.parse(serde_json::json!({"x":1}));
        drop(rec);
        assert!(path.exists());
    }
}
