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
    home_dirs: Vec<std::path::PathBuf>,
    tmp_dirs: Vec<std::path::PathBuf>,
    cwds: Vec<std::path::PathBuf>,
    uuid_counter: std::sync::Mutex<std::collections::HashMap<String, usize>>,
}

fn canonicalized_pair(p: Option<std::path::PathBuf>) -> Vec<std::path::PathBuf> {
    let Some(p) = p else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(2);
    let canon = std::fs::canonicalize(&p).ok();
    if let Some(c) = canon.as_ref() {
        if c != &p {
            out.push(c.clone());
        }
    }
    out.push(p);
    out
}

impl DefaultSanitizer {
    pub fn new() -> Self {
        Self {
            home_dirs: canonicalized_pair(std::env::var("HOME").ok().map(std::path::PathBuf::from)),
            tmp_dirs: canonicalized_pair(Some(std::env::temp_dir())),
            cwds: canonicalized_pair(std::env::current_dir().ok()),
            uuid_counter: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn replace_string(&self, s: &str) -> Option<String> {
        let bytes = s.as_bytes();
        let mut out = String::with_capacity(s.len());
        let mut i = 0;
        let mut any_match = false;

        while i < bytes.len() {
            if let Some((rep, end)) = self.match_at(s, i) {
                out.push_str(&rep);
                i = end;
                any_match = true;
                continue;
            }
            let c_len = s[i..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            out.push_str(&s[i..i + c_len]);
            i += c_len;
        }

        if any_match { Some(out) } else { None }
    }

    fn match_at(&self, s: &str, i: usize) -> Option<(String, usize)> {
        if let Some(end) = Self::scan_uuid(s, i) {
            let candidate = &s[i..end];
            return Some((self.uuid_placeholder(candidate), end));
        }
        if let Some(end) = Self::scan_iso_timestamp(s, i) {
            return Some(("<TIMESTAMP>".to_string(), end));
        }
        if let Some(end) = Self::scan_secret(s, i) {
            return Some(("<REDACTED>".to_string(), end));
        }
        if let Some((replacement, end)) = self.scan_path(s, i) {
            return Some((replacement, end));
        }
        None
    }

    fn uuid_placeholder(&self, candidate: &str) -> String {
        let mut map = self.uuid_counter.lock().unwrap_or_else(|e| e.into_inner());
        let next_id = map.len() + 1;
        let id = *map.entry(candidate.to_string()).or_insert(next_id);
        format!("<UUID:{id}>")
    }

    fn scan_uuid(s: &str, i: usize) -> Option<usize> {
        let bytes = s.as_bytes();
        if i + 36 > bytes.len() {
            return None;
        }
        let positions = [8, 13, 18, 23];
        for &p in &positions {
            if bytes[i + p] != b'-' {
                return None;
            }
        }
        let hex_positions: [(usize, usize); 5] = [(0, 8), (9, 13), (14, 18), (19, 23), (24, 36)];
        for (start, end) in hex_positions {
            for b in &bytes[i + start..i + end] {
                if !(b.is_ascii_hexdigit()) {
                    return None;
                }
            }
        }
        Some(i + 36)
    }

    fn scan_iso_timestamp(s: &str, i: usize) -> Option<usize> {
        let bytes = s.as_bytes();
        if i + 10 > bytes.len() {
            return None;
        }
        if !(bytes[i..i + 4].iter().all(|b| b.is_ascii_digit())) {
            return None;
        }
        if bytes[i + 4] != b'-' {
            return None;
        }
        if !(bytes[i + 5..i + 7].iter().all(|b| b.is_ascii_digit())) {
            return None;
        }
        if bytes[i + 7] != b'-' {
            return None;
        }
        if !(bytes[i + 8..i + 10].iter().all(|b| b.is_ascii_digit())) {
            return None;
        }

        let mut end = i + 10;
        if end < bytes.len()
            && (bytes[end] == b'T' || bytes[end] == b' ')
            && end + 9 <= bytes.len()
            && bytes[end + 1..end + 3].iter().all(|b| b.is_ascii_digit())
            && bytes[end + 3] == b':'
            && bytes[end + 4..end + 6].iter().all(|b| b.is_ascii_digit())
            && bytes[end + 6] == b':'
            && bytes[end + 7..end + 9].iter().all(|b| b.is_ascii_digit())
        {
            end += 9;
            if end < bytes.len() && bytes[end] == b'.' {
                let mut e = end + 1;
                while e < bytes.len() && bytes[e].is_ascii_digit() {
                    e += 1;
                }
                end = e;
            }
            if end < bytes.len() {
                match bytes[end] {
                    b'Z' => end += 1,
                    b'+' | b'-' => {
                        if end + 6 <= bytes.len()
                            && bytes[end + 1..end + 3].iter().all(|b| b.is_ascii_digit())
                            && bytes[end + 3] == b':'
                            && bytes[end + 4..end + 6].iter().all(|b| b.is_ascii_digit())
                        {
                            end += 6;
                        }
                    }
                    _ => {}
                }
            }
        }
        Some(end)
    }

    fn scan_secret(s: &str, i: usize) -> Option<usize> {
        const PREFIXES: &[&str] = &[
            "sk-ant-", "sk_live_", "sk-proj-", "sk-", "AIza", "ghp_", "ghs_", "gho_", "ya29.",
        ];
        let rest = &s[i..];
        for p in PREFIXES {
            if rest.starts_with(p) {
                let mut e = i + p.len();
                while e < s.len() {
                    let c = s.as_bytes()[e];
                    if c.is_ascii_alphanumeric() || c == b'-' || c == b'_' || c == b'.' {
                        e += 1;
                    } else {
                        break;
                    }
                }
                if e - i >= p.len() + 20 {
                    return Some(e);
                }
            }
        }
        None
    }

    fn scan_path(&self, s: &str, i: usize) -> Option<(String, usize)> {
        let candidates: [(&str, &[std::path::PathBuf]); 3] = [
            ("<CWD>", &self.cwds),
            ("<HOME>", &self.home_dirs),
            ("<TMP>", &self.tmp_dirs),
        ];
        for (placeholder, bases) in candidates {
            for base in bases {
                let base = base.to_string_lossy();
                if base.is_empty() {
                    continue;
                }
                if s[i..].starts_with(base.as_ref()) {
                    let after_base = i + base.len();
                    let boundary_ok =
                        after_base == s.len() || matches!(s.as_bytes()[after_base], b'/' | b'\\');
                    if !boundary_ok {
                        continue;
                    }
                    let mut e = after_base;
                    while e < s.len() {
                        let c = s.as_bytes()[e];
                        if c == b' ' || c == b'\t' || c == b'\n' || c == b'"' || c == b'\'' {
                            break;
                        }
                        e += 1;
                    }
                    let suffix = &s[after_base..e];
                    return Some((format!("{placeholder}{suffix}"), e));
                }
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

/// Recorded-test helpers. The `run_or_replay` function switches between
/// replaying a saved fixture (default) and recording a fresh one against
/// the real CLI (when `PILOT_RECORD` env var matches).
pub mod recorded_test {
    use super::{DefaultSanitizer, RecordingDriver};
    use crate::driver::{Driver, TurnInput, TurnOptions};
    use crate::{Session, Turn, TurnItem};
    use futures_core::Stream;
    use std::path::{Path, PathBuf};
    use std::pin::pin;

    /// Whether a scenario should record or replay, based on `PILOT_RECORD`.
    pub enum ScenarioMode {
        Replay,
        Record,
    }

    /// Compute mode for a given fixture path from `PILOT_RECORD`.
    /// - "1" or "all" → Record
    /// - non-empty substring matching fixture_path → Record
    /// - otherwise → Replay
    pub fn mode_for(fixture_path: &Path) -> ScenarioMode {
        let var = std::env::var("PILOT_RECORD").unwrap_or_default();
        if var.is_empty() {
            return ScenarioMode::Replay;
        }
        if var == "1" || var == "all" {
            return ScenarioMode::Record;
        }
        let path_str = fixture_path.to_string_lossy();
        if path_str.contains(&var) {
            ScenarioMode::Record
        } else {
            ScenarioMode::Replay
        }
    }

    /// Run a single-turn scenario in either replay or record mode.
    ///
    /// In **replay** mode (default): read `fixture_path`, parse each line
    /// through `driver_factory().parse()`, accumulate events into a `Turn`,
    /// and return it. No CLI process is spawned.
    ///
    /// In **record** mode (`PILOT_RECORD` env matches): wrap
    /// `driver_factory()` in [`RecordingDriver`] + [`DefaultSanitizer`],
    /// open a real `Session`, send `input` with `opts`, drain the
    /// `TurnStream`, and atomically rename the temp recording into
    /// `fixture_path`. Returns the live-captured `Turn`.
    ///
    /// Panics on error. This is test infrastructure; test failures should be
    /// loud.
    pub async fn run_or_replay<D, F>(
        driver_factory: F,
        input: impl Into<TurnInput>,
        opts: TurnOptions,
        workdir: impl Into<PathBuf>,
        fixture_path: impl AsRef<Path>,
    ) -> Turn
    where
        D: Driver + 'static,
        F: Fn() -> D,
    {
        let fixture_path = fixture_path.as_ref();
        match mode_for(fixture_path) {
            ScenarioMode::Replay => replay(&driver_factory(), fixture_path),
            ScenarioMode::Record => {
                record(
                    &driver_factory,
                    input.into(),
                    opts,
                    workdir.into(),
                    fixture_path,
                )
                .await
            }
        }
    }

    fn replay<D: Driver>(driver: &D, fixture_path: &Path) -> Turn {
        let raw = std::fs::read_to_string(fixture_path).unwrap_or_else(|e| {
            panic!(
                "run_or_replay: cannot read fixture {}: {}. Set PILOT_RECORD=1 to record.",
                fixture_path.display(),
                e
            )
        });
        let mut events: Vec<crate::Event> = Vec::new();
        for (lineno, line) in raw.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let value: serde_json::Value = serde_json::from_str(line).unwrap_or_else(|e| {
                panic!(
                    "run_or_replay: fixture line {} in {} is not valid JSON: {}",
                    lineno + 1,
                    fixture_path.display(),
                    e
                )
            });
            let parsed = driver.parse(value).unwrap_or_else(|e| {
                panic!(
                    "run_or_replay: parse failed on line {} of {}: {:?}",
                    lineno + 1,
                    fixture_path.display(),
                    e
                )
            });
            events.extend(parsed);
        }
        crate::Turn {
            events,
            errors: vec![],
        }
    }

    async fn record<D, F>(
        driver_factory: &F,
        input: TurnInput,
        opts: TurnOptions,
        workdir: PathBuf,
        fixture_path: &Path,
    ) -> Turn
    where
        D: Driver + 'static,
        F: Fn() -> D,
    {
        if let Some(parent) = fixture_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).unwrap_or_else(|e| {
                    panic!(
                        "run_or_replay: cannot create fixture parent dir {}: {}",
                        parent.display(),
                        e
                    )
                });
            }
        }
        let tmp_path = {
            let mut p = fixture_path.to_path_buf();
            let stem = fixture_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            p.set_file_name(format!(".{stem}.recording"));
            p
        };

        let rec = RecordingDriver::new(driver_factory(), &tmp_path)
            .unwrap_or_else(|e| panic!("run_or_replay: cannot open recording temp file: {e}"))
            .with_sanitizer(DefaultSanitizer::new());
        let signal = rec.failure_signal();

        let mut session = Session::new(rec, workdir);
        let stream = session
            .send(input, opts)
            .await
            .unwrap_or_else(|e| panic!("run_or_replay: session.send failed: {e:?}"));

        let mut events: Vec<crate::Event> = Vec::new();
        let mut stream = pin!(stream);
        loop {
            let next = std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await;
            match next {
                None => break,
                Some(Err(e)) => {
                    panic!("run_or_replay: stream error during recording: {e:?}")
                }
                Some(Ok(TurnItem::Event(e))) => events.push(e),
                Some(Ok(TurnItem::Complete(_))) => {}
            }
        }

        if signal.load(std::sync::atomic::Ordering::SeqCst) {
            panic!(
                "run_or_replay: recording reported write/serialization failures (see tracing logs). Temp file kept at {} for inspection.",
                tmp_path.display()
            );
        }

        std::fs::rename(&tmp_path, fixture_path).unwrap_or_else(|e| {
            panic!(
                "run_or_replay: cannot rename {} -> {}: {}",
                tmp_path.display(),
                fixture_path.display(),
                e
            )
        });

        crate::Turn {
            events,
            errors: vec![],
        }
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
        s.home_dirs = vec![std::path::PathBuf::from("/Users/test")];
        s.tmp_dirs = Vec::new();
        s.cwds = Vec::new();
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
    fn default_sanitizer_handles_embedded_uuid() {
        let s = DefaultSanitizer::new();
        let mut v =
            serde_json::json!("error at session 11111111-1111-1111-1111-111111111111 happened");
        s.sanitize(&mut v);
        assert_eq!(v, serde_json::json!("error at session <UUID:1> happened"));
    }

    #[test]
    fn default_sanitizer_handles_embedded_timestamp() {
        let s = DefaultSanitizer::new();
        let mut v = serde_json::json!("started at 2026-05-18T12:34:56.789Z and finished");
        s.sanitize(&mut v);
        assert_eq!(v, serde_json::json!("started at <TIMESTAMP> and finished"));
    }

    #[test]
    fn default_sanitizer_handles_embedded_secret() {
        let s = DefaultSanitizer::new();
        let mut v =
            serde_json::json!("Authorization: Bearer sk-ant-abcdefghijklmnopqrstuvwxyz0123");
        s.sanitize(&mut v);
        assert_eq!(v, serde_json::json!("Authorization: Bearer <REDACTED>"));
    }

    #[test]
    fn default_sanitizer_handles_embedded_path() {
        let mut s = DefaultSanitizer::new();
        s.home_dirs = vec![std::path::PathBuf::from("/Users/test")];
        s.tmp_dirs = Vec::new();
        s.cwds = Vec::new();
        let mut v = serde_json::json!("file is at /Users/test/project/foo.rs in the repo");
        s.sanitize(&mut v);
        assert_eq!(
            v,
            serde_json::json!("file is at <HOME>/project/foo.rs in the repo")
        );
    }

    #[test]
    fn default_sanitizer_path_boundary_prevents_partial_match() {
        let mut s = DefaultSanitizer::new();
        s.home_dirs = vec![std::path::PathBuf::from("/Users/test")];
        s.tmp_dirs = Vec::new();
        s.cwds = Vec::new();
        let mut v = serde_json::json!({
            "exact": "/Users/test",
            "subpath": "/Users/test/foo",
            "looks_similar": "/Users/tester/foo",
        });
        s.sanitize(&mut v);
        assert_eq!(v["exact"], "<HOME>");
        assert_eq!(v["subpath"], "<HOME>/foo");
        assert_eq!(v["looks_similar"], "/Users/tester/foo");
    }

    #[test]
    fn default_sanitizer_accepts_backslash_as_path_separator() {
        let mut s = DefaultSanitizer::new();
        s.home_dirs = vec![std::path::PathBuf::from(r"C:\Users\test")];
        s.tmp_dirs = Vec::new();
        s.cwds = Vec::new();
        let mut v = serde_json::json!({
            "subpath": r"C:\Users\test\project\foo.rs",
            "looks_similar": r"C:\Users\tester\foo.rs",
        });
        s.sanitize(&mut v);
        assert_eq!(v["subpath"], r"<HOME>\project\foo.rs");
        assert_eq!(v["looks_similar"], r"C:\Users\tester\foo.rs");
    }

    #[test]
    fn default_sanitizer_matches_canonical_paths_on_macos_style_links() {
        let mut s = DefaultSanitizer::new();
        s.home_dirs = Vec::new();
        s.cwds = Vec::new();
        s.tmp_dirs = vec![
            std::path::PathBuf::from("/private/var/folders/5k/abc/T"),
            std::path::PathBuf::from("/var/folders/5k/abc/T"),
        ];
        let mut v = serde_json::json!({
            "canonical": "/private/var/folders/5k/abc/T/foo",
            "symlinked": "/var/folders/5k/abc/T/bar",
        });
        s.sanitize(&mut v);
        assert_eq!(v["canonical"], "<TMP>/foo");
        assert_eq!(v["symlinked"], "<TMP>/bar");
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

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn run_or_replay_replays_fixture_against_test_driver() {
        use recorded_test::run_or_replay;
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let tmp_dir = tempfile::tempdir().unwrap();
        let fixture = tmp_dir.path().join("test.jsonl");
        std::fs::write(
            &fixture,
            r#"{"type":"a","x":1}
{"type":"b","x":2}
"#,
        )
        .unwrap();
        unsafe {
            std::env::remove_var("PILOT_RECORD");
        }

        let turn = run_or_replay(
            || TestDriver::new("t", "/bin/echo"),
            "ignored-input",
            TurnOptions::default(),
            "/tmp",
            &fixture,
        )
        .await;

        assert_eq!(turn.events.len(), 2);
        assert!(matches!(
            &turn.events[0],
            crate::Event::Raw { driver: "t", .. }
        ));
    }

    #[tokio::test]
    async fn mode_for_respects_pilot_record_substring() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var("PILOT_RECORD", "claude/invalid");
        }
        assert!(matches!(
            recorded_test::mode_for(std::path::Path::new("fixtures/claude/invalid_model.jsonl")),
            recorded_test::ScenarioMode::Record
        ));
        assert!(matches!(
            recorded_test::mode_for(std::path::Path::new("fixtures/codex/greeting.jsonl")),
            recorded_test::ScenarioMode::Replay
        ));
        unsafe {
            std::env::remove_var("PILOT_RECORD");
        }
    }

    #[tokio::test]
    async fn mode_for_unset_returns_replay() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::remove_var("PILOT_RECORD");
        }
        assert!(matches!(
            recorded_test::mode_for(std::path::Path::new("fixtures/x.jsonl")),
            recorded_test::ScenarioMode::Replay
        ));
    }

    #[tokio::test]
    async fn mode_for_all_returns_record() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe {
            std::env::set_var("PILOT_RECORD", "all");
        }
        assert!(matches!(
            recorded_test::mode_for(std::path::Path::new("fixtures/any.jsonl")),
            recorded_test::ScenarioMode::Record
        ));
        unsafe {
            std::env::remove_var("PILOT_RECORD");
        }
    }
}
