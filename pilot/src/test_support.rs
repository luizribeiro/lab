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

/// Mutates JSON values before they're written to a fixture file by
/// [`RecordingDriver`]. Used to scrub non-deterministic and host-specific
/// data so captures are stable and committable.
pub trait Sanitizer: Send + Sync {
    fn sanitize(&self, value: &mut serde_json::Value);
}

/// Built-in sanitizer that scrubs UUIDs, timestamps, absolute paths, and
/// common secret patterns.
pub struct DefaultSanitizer {
    home_dir: Option<std::path::PathBuf>,
    tmp_dir: Option<std::path::PathBuf>,
    cwd: Option<std::path::PathBuf>,
    uuid_counter: std::sync::Mutex<std::collections::HashMap<String, usize>>,
}

impl DefaultSanitizer {
    pub fn new() -> Self {
        Self {
            home_dir: std::env::var("HOME").ok().map(std::path::PathBuf::from),
            tmp_dir: Some(std::env::temp_dir()),
            cwd: std::env::current_dir().ok(),
            uuid_counter: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn replace_string(&self, s: &str) -> Option<String> {
        if uuid::Uuid::parse_str(s).is_ok() {
            let mut map = self.uuid_counter.lock().unwrap_or_else(|e| e.into_inner());
            let next_id = map.len() + 1;
            let id = *map.entry(s.to_string()).or_insert(next_id);
            return Some(format!("<UUID:{id}>"));
        }

        if Self::looks_like_iso_timestamp(s) {
            return Some("<TIMESTAMP>".to_string());
        }

        if let Some(replaced) = self.replace_path_prefix(s) {
            return Some(replaced);
        }

        if Self::looks_like_secret(s) {
            return Some("<REDACTED>".to_string());
        }

        None
    }

    fn looks_like_iso_timestamp(s: &str) -> bool {
        if s.len() < 10 {
            return false;
        }
        let bytes = s.as_bytes();
        bytes[4] == b'-'
            && bytes[7] == b'-'
            && s[..4].chars().all(|c| c.is_ascii_digit())
            && s[5..7].chars().all(|c| c.is_ascii_digit())
            && s[8..10].chars().all(|c| c.is_ascii_digit())
            && (s.len() == 10 || bytes[10] == b'T' || bytes[10] == b' ')
    }

    fn looks_like_secret(s: &str) -> bool {
        const PREFIXES: &[&str] = &[
            "sk-ant-", "sk_live_", "sk-proj-", "sk-", "AIza", "ghp_", "ghs_", "gho_", "ya29.",
        ];
        for p in PREFIXES {
            if s.starts_with(p) && s.len() >= p.len() + 20 {
                return true;
            }
        }
        false
    }

    fn replace_path_prefix(&self, s: &str) -> Option<String> {
        for (placeholder, base) in [
            ("<CWD>", self.cwd.as_deref()),
            ("<HOME>", self.home_dir.as_deref()),
            ("<TMP>", self.tmp_dir.as_deref()),
        ] {
            let base = match base {
                Some(b) => b.to_string_lossy(),
                None => continue,
            };
            if base.is_empty() {
                continue;
            }
            if s.starts_with(base.as_ref()) {
                return Some(format!("{placeholder}{}", &s[base.len()..]));
            }
        }
        None
    }
}

impl Default for DefaultSanitizer {
    fn default() -> Self {
        Self::new()
    }
}

impl Sanitizer for DefaultSanitizer {
    fn sanitize(&self, value: &mut serde_json::Value) {
        walk_strings(value, &|s| self.replace_string(s));
    }
}

fn walk_strings(value: &mut serde_json::Value, replace: &impl Fn(&str) -> Option<String>) {
    match value {
        serde_json::Value::String(s) => {
            if let Some(new) = replace(s) {
                *s = new;
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                walk_strings(v, replace);
            }
        }
        serde_json::Value::Object(map) => {
            for (_, v) in map.iter_mut() {
                walk_strings(v, replace);
            }
        }
        _ => {}
    }
}

/// A `Driver` wrapper that tees every raw JSON value to a file before
/// forwarding to the inner driver's `parse`.
pub struct RecordingDriver<D: Driver> {
    inner: D,
    file: Mutex<File>,
    recording_failed: Arc<AtomicBool>,
    sanitizer: Option<Box<dyn Sanitizer>>,
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
            sanitizer: None,
        })
    }

    /// Attach a sanitizer that mutates each JSON value before it's written
    /// to the fixture file. The inner driver still receives the unmodified
    /// value via `parse()`.
    pub fn with_sanitizer(mut self, sanitizer: impl Sanitizer + 'static) -> Self {
        self.sanitizer = Some(Box::new(sanitizer));
        self
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
        let to_write = if let Some(s) = &self.sanitizer {
            let mut clone = value.clone();
            s.sanitize(&mut clone);
            clone
        } else {
            value.clone()
        };
        match serde_json::to_string(&to_write) {
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
    fn default_sanitizer_replaces_uuids_stably() {
        let s = DefaultSanitizer::new();
        let mut v = serde_json::json!({
            "id": "11111111-1111-1111-1111-111111111111",
            "other_id": "22222222-2222-2222-2222-222222222222",
            "same_id_again": "11111111-1111-1111-1111-111111111111",
        });
        s.sanitize(&mut v);
        assert_eq!(v["id"], "<UUID:1>");
        assert_eq!(v["other_id"], "<UUID:2>");
        assert_eq!(v["same_id_again"], "<UUID:1>");
    }

    #[test]
    fn default_sanitizer_replaces_iso_timestamps() {
        let s = DefaultSanitizer::new();
        let mut v = serde_json::json!({
            "ts1": "2026-05-18T12:34:56.789Z",
            "ts2": "2026-05-18 12:34:56",
            "not_ts": "hello world",
            "not_ts2": "1234-foo-bar",
        });
        s.sanitize(&mut v);
        assert_eq!(v["ts1"], "<TIMESTAMP>");
        assert_eq!(v["ts2"], "<TIMESTAMP>");
        assert_eq!(v["not_ts"], "hello world");
        assert_eq!(v["not_ts2"], "1234-foo-bar");
    }

    #[test]
    fn default_sanitizer_redacts_secret_prefixes() {
        let s = DefaultSanitizer::new();
        let mut v = serde_json::json!({
            "key1": "sk-ant-abcdefghijklmnopqrstuvwxyz0123456789",
            "key2": "AIzaSyAabcdefghijklmnopqrstuvwxyz0123456",
            "short_ok": "sk-too-short",
            "innocuous": "skip-this",
        });
        s.sanitize(&mut v);
        assert_eq!(v["key1"], "<REDACTED>");
        assert_eq!(v["key2"], "<REDACTED>");
        assert_eq!(v["short_ok"], "sk-too-short");
        assert_eq!(v["innocuous"], "skip-this");
    }

    #[test]
    fn default_sanitizer_replaces_home_path_prefix() {
        let mut s = DefaultSanitizer::new();
        s.home_dir = Some(std::path::PathBuf::from("/Users/test"));
        s.tmp_dir = None;
        s.cwd = None;
        let mut v = serde_json::json!({
            "path": "/Users/test/.claude/config.toml",
            "external": "/etc/passwd",
        });
        s.sanitize(&mut v);
        assert_eq!(v["path"], "<HOME>/.claude/config.toml");
        assert_eq!(v["external"], "/etc/passwd");
    }

    #[test]
    fn recording_driver_sanitizes_before_writing() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let inner = TestDriver::new("inner", "/bin/echo");
        let rec = RecordingDriver::new(inner, tmp.path())
            .unwrap()
            .with_sanitizer(DefaultSanitizer::new());

        let v = serde_json::json!({
            "ts": "2026-01-01T00:00:00Z",
            "id": "33333333-3333-3333-3333-333333333333",
        });
        let evs = rec.parse(v.clone()).unwrap();
        drop(rec);

        let content = std::fs::read_to_string(tmp.path()).unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(content.lines().next().unwrap()).unwrap();
        assert_eq!(parsed["ts"], "<TIMESTAMP>");
        assert_eq!(parsed["id"], "<UUID:1>");

        if let Event::Raw { value: passed, .. } = &evs[0] {
            assert_eq!(passed["ts"], "2026-01-01T00:00:00Z");
            assert_eq!(passed["id"], "33333333-3333-3333-3333-333333333333");
        } else {
            panic!("expected Raw event");
        }
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
