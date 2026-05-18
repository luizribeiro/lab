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
/// [`Cassette`] in Record mode. Used to scrub non-deterministic and
/// host-specific data so captures are stable and committable.
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

/// Whether a [`Cassette`] is replaying a fixture or recording a fresh one.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CassetteMode {
    Replay,
    Record,
}

/// A `Driver` wrapper that switches between replaying a fixture file
/// (via `cat <fixture>` through pilot's normal spawn pipeline) and
/// recording a fresh fixture against the inner driver's real CLI.
///
/// Mode is resolved at construction by [`Cassette::auto`]:
/// - `PILOT_NO_RECORD=1`: fixture must exist (else panic) → Replay
/// - `PILOT_RECORD` matches (per `pilot_record_matches` rules) → Record
/// - fixture exists → Replay
/// - fixture missing → Record (first-run capture)
///
/// In Record mode, written lines are sanitized (default
/// [`DefaultSanitizer`]) and buffered to `<fixture_path>.recording`.
/// On `Drop`, if no failures were flagged AND at least one line was
/// written, the temp file is atomically renamed to `fixture_path`.
/// On failure, the `.recording` file is kept for inspection.
pub struct Cassette<D: Driver> {
    inner: D,
    fixture_path: PathBuf,
    tmp_path: PathBuf,
    mode: CassetteMode,
    file: Option<Mutex<File>>,
    sanitizer: Option<Box<dyn Sanitizer>>,
    recording_failed: Arc<AtomicBool>,
    lines_written: Arc<std::sync::atomic::AtomicUsize>,
}

fn pilot_record_matches(fixture_path: &std::path::Path) -> bool {
    let v = std::env::var("PILOT_RECORD").unwrap_or_default();
    if v.is_empty() {
        return false;
    }
    if v == "1" || v == "all" {
        return true;
    }
    fixture_path.to_string_lossy().contains(&v)
}

impl<D: Driver> Cassette<D> {
    /// Build a `Cassette` whose mode is selected automatically (see
    /// [`Cassette`] doc for the resolution table).
    ///
    /// In Record mode this creates the parent directory and opens
    /// `<fixture_path>.recording` for writing. Panics if either fails —
    /// this is test infrastructure; test failures should be loud.
    pub fn auto(inner: D, fixture_path: impl Into<PathBuf>) -> Self {
        let fixture_path = fixture_path.into();
        let exists = fixture_path.exists();
        let no_record = std::env::var("PILOT_NO_RECORD")
            .ok()
            .filter(|v| v == "1")
            .is_some();

        let mode = if no_record {
            if !exists {
                panic!(
                    "fixture not found; PILOT_NO_RECORD prohibits recording. Path: {}",
                    fixture_path.display()
                );
            }
            CassetteMode::Replay
        } else if pilot_record_matches(&fixture_path) {
            CassetteMode::Record
        } else if exists {
            CassetteMode::Replay
        } else {
            CassetteMode::Record
        };

        let tmp_path = {
            let stem = fixture_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut p = fixture_path.clone();
            p.set_file_name(format!(".{stem}.recording"));
            p
        };

        let (file, sanitizer): (Option<Mutex<File>>, Option<Box<dyn Sanitizer>>) =
            if matches!(mode, CassetteMode::Record) {
                if let Some(parent) = fixture_path.parent() {
                    if !parent.as_os_str().is_empty() {
                        std::fs::create_dir_all(parent).unwrap_or_else(|e| {
                            panic!(
                                "Cassette: cannot create parent dir {}: {e}",
                                parent.display()
                            )
                        });
                    }
                }
                let f = File::create(&tmp_path).unwrap_or_else(|e| {
                    panic!(
                        "Cassette: cannot open recording temp file {}: {e}",
                        tmp_path.display()
                    )
                });
                (Some(Mutex::new(f)), Some(Box::new(DefaultSanitizer::new())))
            } else {
                (None, None)
            };

        Self {
            inner,
            fixture_path,
            tmp_path,
            mode,
            file,
            sanitizer,
            recording_failed: Arc::new(AtomicBool::new(false)),
            lines_written: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Override the default record-mode sanitizer. No-op in Replay mode.
    pub fn with_sanitizer(mut self, sanitizer: impl Sanitizer + 'static) -> Self {
        if matches!(self.mode, CassetteMode::Record) {
            self.sanitizer = Some(Box::new(sanitizer));
        }
        self
    }

    pub fn mode(&self) -> CassetteMode {
        self.mode
    }

    pub fn recording_failed(&self) -> bool {
        self.recording_failed.load(Ordering::SeqCst)
    }

    pub fn failure_signal(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.recording_failed)
    }
}

impl<D: Driver> Driver for Cassette<D> {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn command(
        &self,
        session_id: Uuid,
        input: &TurnInput,
        opts: &TurnOptions,
    ) -> crate::Result<CommandSpec> {
        match self.mode {
            CassetteMode::Replay => Ok(CommandSpec {
                program: PathBuf::from("cat"),
                args: vec![self.fixture_path.to_string_lossy().into_owned()],
                env: Vec::new(),
            }),
            CassetteMode::Record => self.inner.command(session_id, input, opts),
        }
    }

    fn resume_command(
        &self,
        session_id: Uuid,
        input: &TurnInput,
        opts: &TurnOptions,
    ) -> crate::Result<CommandSpec> {
        match self.mode {
            CassetteMode::Replay => Ok(CommandSpec {
                program: PathBuf::from("cat"),
                args: vec![self.fixture_path.to_string_lossy().into_owned()],
                env: Vec::new(),
            }),
            CassetteMode::Record => self.inner.resume_command(session_id, input, opts),
        }
    }

    fn observe(&self, session_id: Uuid, raw: &serde_json::Value) {
        if matches!(self.mode, CassetteMode::Record) {
            self.inner.observe(session_id, raw);
        }
    }

    fn parse(&self, value: serde_json::Value) -> std::result::Result<Vec<Event>, ParseError> {
        if matches!(self.mode, CassetteMode::Record) {
            let to_write = if let Some(s) = &self.sanitizer {
                let mut clone = value.clone();
                s.sanitize(&mut clone);
                clone
            } else {
                value.clone()
            };
            match serde_json::to_string(&to_write) {
                Ok(line) => match self.file.as_ref().expect("file open in record mode").lock() {
                    Ok(mut f) => {
                        if let Err(e) = writeln!(f, "{line}") {
                            tracing::warn!(error = %e, "Cassette: file write failed");
                            self.recording_failed.store(true, Ordering::SeqCst);
                        } else {
                            self.lines_written.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Cassette: mutex poisoned");
                        self.recording_failed.store(true, Ordering::SeqCst);
                    }
                },
                Err(e) => {
                    tracing::warn!(error = %e, "Cassette: serialization failed");
                    self.recording_failed.store(true, Ordering::SeqCst);
                }
            }
        }
        self.inner.parse(value)
    }
}

impl<D: Driver> Drop for Cassette<D> {
    fn drop(&mut self) {
        if !matches!(self.mode, CassetteMode::Record) {
            return;
        }
        self.file = None;
        if self.recording_failed.load(Ordering::SeqCst) {
            return;
        }
        if self.lines_written.load(Ordering::SeqCst) == 0 {
            return;
        }
        if let Err(e) = std::fs::rename(&self.tmp_path, &self.fixture_path) {
            tracing::warn!(error = %e, "Cassette: rename failed");
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

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn clear_pilot_env() {
        unsafe {
            std::env::remove_var("PILOT_RECORD");
            std::env::remove_var("PILOT_NO_RECORD");
        }
    }

    #[test]
    fn cassette_mode_record_when_fixture_missing() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_pilot_env();
        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("missing.jsonl");
        let c = Cassette::auto(TestDriver::new("t", "/bin/echo"), &fixture);
        assert!(matches!(c.mode(), CassetteMode::Record));
    }

    #[test]
    fn cassette_mode_replay_when_fixture_exists() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_pilot_env();
        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("exists.jsonl");
        std::fs::write(&fixture, "{}\n").unwrap();
        let c = Cassette::auto(TestDriver::new("t", "/bin/echo"), &fixture);
        assert!(matches!(c.mode(), CassetteMode::Replay));
    }

    #[test]
    fn cassette_no_record_panics_when_fixture_missing() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_pilot_env();
        unsafe {
            std::env::set_var("PILOT_NO_RECORD", "1");
        }
        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("missing.jsonl");
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = Cassette::auto(TestDriver::new("t", "/bin/echo"), &fixture);
        }));
        unsafe {
            std::env::remove_var("PILOT_NO_RECORD");
        }
        assert!(
            result.is_err(),
            "expected panic when fixture missing under PILOT_NO_RECORD"
        );
    }

    #[test]
    fn cassette_record_writes_fixture_and_atomically_renames() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_pilot_env();
        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("foo.jsonl");
        let tmp = dir.path().join(".foo.jsonl.recording");
        {
            let cassette = Cassette::auto(TestDriver::new("t", "/bin/echo"), &fixture);
            assert!(matches!(cassette.mode(), CassetteMode::Record));
            assert!(tmp.exists(), "expected tmp file to exist during recording");
            let _ = cassette.parse(serde_json::json!({"a": 1})).unwrap();
            let _ = cassette.parse(serde_json::json!({"a": 2})).unwrap();
        }
        assert!(fixture.exists(), "fixture should exist after drop");
        assert!(!tmp.exists(), "tmp should be gone after rename");
        let content = std::fs::read_to_string(&fixture).unwrap();
        assert_eq!(content.lines().count(), 2);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn cassette_replay_runs_fixture_through_pilot_pipeline() {
        use futures_core::Stream;
        use std::pin::pin;

        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        clear_pilot_env();
        let dir = tempfile::tempdir().unwrap();
        let fixture = dir.path().join("replay.jsonl");
        std::fs::write(&fixture, "{\"x\":1}\n{\"x\":2}\n").unwrap();
        let driver = Cassette::auto(TestDriver::new("t", "/bin/echo"), &fixture);
        assert!(matches!(driver.mode(), CassetteMode::Replay));

        let mut session = crate::Session::new(driver, "/tmp");
        let stream = session
            .send("ignored", TurnOptions::default())
            .await
            .unwrap();
        let mut stream = pin!(stream);
        let mut events: Vec<Event> = Vec::new();
        loop {
            let next = std::future::poll_fn(|cx| stream.as_mut().poll_next(cx)).await;
            match next {
                None => break,
                Some(Ok(crate::TurnItem::Event(e))) => events.push(e),
                Some(Ok(crate::TurnItem::Complete(_))) => {}
                Some(Err(e)) => panic!("stream error: {e:?}"),
            }
        }
        assert_eq!(events.len(), 2);
    }
}
