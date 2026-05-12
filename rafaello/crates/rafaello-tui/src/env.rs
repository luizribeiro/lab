//! Environment-variable parsing for the `rfl-tui` binary (scope §T2 step 1).

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use serde_json::Value;

pub const RFL_BUS_FD: &str = "RFL_BUS_FD";
pub const RFL_PROJECT_ROOT: &str = "RFL_PROJECT_ROOT";
pub const RFL_TUI_TEST_MODE: &str = "RFL_TUI_TEST_MODE";
pub const RFL_TUI_READY_DELAY_MS: &str = "RFL_TUI_READY_DELAY_MS";
pub const RFL_TUI_MAX_LIFETIME: &str = "RFL_TUI_MAX_LIFETIME";
pub const RFL_TUI_TEST_MESSAGE: &str = "RFL_TUI_TEST_MESSAGE";
pub const RFL_TUI_TEST_CONFIRM_ANSWER: &str = "RFL_TUI_TEST_CONFIRM_ANSWER";
pub const RFL_TUI_TEST_CONFIRM_ANSWERS: &str = "RFL_TUI_TEST_CONFIRM_ANSWERS";
pub const RFL_TUI_TEST_CONFIRM_DELAY_MS: &str = "RFL_TUI_TEST_CONFIRM_DELAY_MS";
pub const RFL_TUI_TEST_GRANT_BEFORE_MESSAGE: &str = "RFL_TUI_TEST_GRANT_BEFORE_MESSAGE";

pub const ENV_PASS_ALLOWLIST: &[&str] = &[
    RFL_BUS_FD,
    RFL_PROJECT_ROOT,
    RFL_TUI_TEST_MODE,
    RFL_TUI_READY_DELAY_MS,
    RFL_TUI_MAX_LIFETIME,
    RFL_TUI_TEST_MESSAGE,
    RFL_TUI_TEST_CONFIRM_ANSWER,
    RFL_TUI_TEST_CONFIRM_ANSWERS,
    RFL_TUI_TEST_CONFIRM_DELAY_MS,
    RFL_TUI_TEST_GRANT_BEFORE_MESSAGE,
];

pub const MUTUALLY_EXCLUSIVE_CONFIRM_ANSWER_ERR: &str =
    "RFL_TUI_TEST_CONFIRM_ANSWER and RFL_TUI_TEST_CONFIRM_ANSWERS are mutually exclusive; \
     set one or the other";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestConfirmAnswer {
    Allow,
    Deny,
    AlwaysAllowSession,
    Timeout,
}

impl TestConfirmAnswer {
    pub fn answer_str(self) -> Option<&'static str> {
        match self {
            TestConfirmAnswer::Allow => Some("allow"),
            TestConfirmAnswer::Deny => Some("deny"),
            TestConfirmAnswer::AlwaysAllowSession => Some("always_allow_session"),
            TestConfirmAnswer::Timeout => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TestGrantBeforeMessage {
    pub tool: String,
    pub args_subset: Value,
}

#[derive(Debug, Clone)]
pub struct TuiEnv {
    pub bus_fd: i32,
    pub project_root: PathBuf,
    pub test_mode: bool,
    pub ready_delay_ms: Option<u64>,
    pub max_lifetime_secs: Option<u64>,
    pub test_message: Option<String>,
    pub test_confirm_answer: Option<TestConfirmAnswer>,
    pub test_confirm_answers: Option<Vec<TestConfirmAnswer>>,
    pub test_confirm_delay_ms: u64,
    pub test_grant_before_message: Option<TestGrantBeforeMessage>,
}

pub fn load() -> Result<TuiEnv> {
    load_from(|key| std::env::var(key).ok())
}

pub fn load_from<F>(get: F) -> Result<TuiEnv>
where
    F: Fn(&str) -> Option<String>,
{
    let bus_fd = parse_bus_fd(get(RFL_BUS_FD))?;
    let project_root = parse_project_root(get(RFL_PROJECT_ROOT))?;
    let test_mode = get(RFL_TUI_TEST_MODE).as_deref() == Some("1");
    let ready_delay_ms = parse_optional_u64(RFL_TUI_READY_DELAY_MS, get(RFL_TUI_READY_DELAY_MS))?;
    let max_lifetime_secs = parse_optional_u64(RFL_TUI_MAX_LIFETIME, get(RFL_TUI_MAX_LIFETIME))?;
    let test_message = get(RFL_TUI_TEST_MESSAGE).filter(|s| !s.is_empty());
    let test_confirm_answer = parse_confirm_answer(get(RFL_TUI_TEST_CONFIRM_ANSWER))?;
    let test_confirm_answers = parse_confirm_answers_env(get(RFL_TUI_TEST_CONFIRM_ANSWERS))?;
    if test_confirm_answer.is_some() && test_confirm_answers.is_some() {
        return Err(anyhow!("{}", MUTUALLY_EXCLUSIVE_CONFIRM_ANSWER_ERR));
    }
    let test_confirm_delay_ms = parse_optional_u64(
        RFL_TUI_TEST_CONFIRM_DELAY_MS,
        get(RFL_TUI_TEST_CONFIRM_DELAY_MS),
    )?
    .unwrap_or(0);
    let test_grant_before_message =
        parse_grant_before_message(get(RFL_TUI_TEST_GRANT_BEFORE_MESSAGE))?;

    Ok(TuiEnv {
        bus_fd,
        project_root,
        test_mode,
        ready_delay_ms,
        max_lifetime_secs,
        test_message,
        test_confirm_answer,
        test_confirm_answers,
        test_confirm_delay_ms,
        test_grant_before_message,
    })
}

fn parse_confirm_answer_token(token: &str) -> Result<TestConfirmAnswer> {
    match token {
        "allow" => Ok(TestConfirmAnswer::Allow),
        "deny" => Ok(TestConfirmAnswer::Deny),
        "always_allow_session" => Ok(TestConfirmAnswer::AlwaysAllowSession),
        "timeout" => Ok(TestConfirmAnswer::Timeout),
        other => Err(anyhow!(
            "confirm-answer entries must be one of allow|deny|always_allow_session|timeout \
             (got {:?})",
            other
        )),
    }
}

fn parse_confirm_answer(value: Option<String>) -> Result<Option<TestConfirmAnswer>> {
    match value.filter(|s| !s.is_empty()).as_deref() {
        None => Ok(None),
        Some(s) => Ok(Some(parse_confirm_answer_token(s)?)),
    }
}

pub fn parse_confirm_answers(s: &str) -> Result<Vec<TestConfirmAnswer>> {
    s.split(',').map(parse_confirm_answer_token).collect()
}

fn parse_confirm_answers_env(value: Option<String>) -> Result<Option<Vec<TestConfirmAnswer>>> {
    match value.filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(raw) => Ok(Some(parse_confirm_answers(&raw)?)),
    }
}

fn parse_grant_before_message(value: Option<String>) -> Result<Option<TestGrantBeforeMessage>> {
    let Some(raw) = value.filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let v: Value = serde_json::from_str(&raw)
        .with_context(|| format!("{} must be valid JSON", RFL_TUI_TEST_GRANT_BEFORE_MESSAGE))?;
    let tool = v
        .get("tool")
        .and_then(|x| x.as_str())
        .ok_or_else(|| {
            anyhow!(
                "{} requires string field `tool`",
                RFL_TUI_TEST_GRANT_BEFORE_MESSAGE
            )
        })?
        .to_string();
    let args_subset = v.get("args_subset").cloned().ok_or_else(|| {
        anyhow!(
            "{} requires field `args_subset`",
            RFL_TUI_TEST_GRANT_BEFORE_MESSAGE
        )
    })?;
    Ok(Some(TestGrantBeforeMessage { tool, args_subset }))
}

fn parse_bus_fd(value: Option<String>) -> Result<i32> {
    let raw = value
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("{} is required", RFL_BUS_FD))?;
    let fd: i32 = raw.parse().with_context(|| {
        format!(
            "{} must be a non-negative integer (got {:?})",
            RFL_BUS_FD, raw
        )
    })?;
    if fd < 0 {
        return Err(anyhow!(
            "{} must be a non-negative integer (got {})",
            RFL_BUS_FD,
            fd
        ));
    }
    Ok(fd)
}

fn parse_project_root(value: Option<String>) -> Result<PathBuf> {
    let raw = value
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow!("{} is required", RFL_PROJECT_ROOT))?;
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(anyhow!(
            "{} must be an absolute path (got {})",
            RFL_PROJECT_ROOT,
            path.display()
        ));
    }
    Ok(path)
}

fn parse_optional_u64(name: &str, value: Option<String>) -> Result<Option<u64>> {
    match value.filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(raw) => {
            let n: u64 = raw.parse().with_context(|| {
                format!("{} must be a non-negative integer (got {:?})", name, raw)
            })?;
            Ok(Some(n))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn getter(map: HashMap<&'static str, &'static str>) -> impl Fn(&str) -> Option<String> {
        move |k| map.get(k).map(|v| v.to_string())
    }

    #[test]
    fn parses_required_and_optional_fields() {
        let env = load_from(getter(HashMap::from([
            (RFL_BUS_FD, "3"),
            (RFL_PROJECT_ROOT, "/abs/path"),
            (RFL_TUI_TEST_MODE, "1"),
            (RFL_TUI_READY_DELAY_MS, "200"),
            (RFL_TUI_MAX_LIFETIME, "60"),
        ])))
        .expect("parse");

        assert_eq!(env.bus_fd, 3);
        assert_eq!(env.project_root, PathBuf::from("/abs/path"));
        assert!(env.test_mode);
        assert_eq!(env.ready_delay_ms, Some(200));
        assert_eq!(env.max_lifetime_secs, Some(60));
    }

    #[test]
    fn defaults_optional_fields() {
        let env = load_from(getter(HashMap::from([
            (RFL_BUS_FD, "3"),
            (RFL_PROJECT_ROOT, "/abs/path"),
        ])))
        .expect("parse");

        assert!(!env.test_mode);
        assert_eq!(env.ready_delay_ms, None);
        assert_eq!(env.max_lifetime_secs, None);
    }

    #[test]
    fn test_mode_only_enabled_when_eq_one() {
        let env = load_from(getter(HashMap::from([
            (RFL_BUS_FD, "3"),
            (RFL_PROJECT_ROOT, "/abs/path"),
            (RFL_TUI_TEST_MODE, "0"),
        ])))
        .expect("parse");
        assert!(!env.test_mode);
    }

    #[test]
    fn rejects_missing_bus_fd() {
        let err = load_from(getter(HashMap::from([(RFL_PROJECT_ROOT, "/abs")]))).unwrap_err();
        assert!(err.to_string().contains(RFL_BUS_FD));
    }

    #[test]
    fn rejects_negative_bus_fd() {
        let err = load_from(getter(HashMap::from([
            (RFL_BUS_FD, "-1"),
            (RFL_PROJECT_ROOT, "/abs"),
        ])))
        .unwrap_err();
        assert!(err.to_string().contains(RFL_BUS_FD));
    }

    #[test]
    fn rejects_relative_project_root() {
        let err = load_from(getter(HashMap::from([
            (RFL_BUS_FD, "3"),
            (RFL_PROJECT_ROOT, "relative/path"),
        ])))
        .unwrap_err();
        assert!(err.to_string().contains(RFL_PROJECT_ROOT));
    }

    #[test]
    fn rejects_non_numeric_ready_delay() {
        let err = load_from(getter(HashMap::from([
            (RFL_BUS_FD, "3"),
            (RFL_PROJECT_ROOT, "/abs"),
            (RFL_TUI_READY_DELAY_MS, "soon"),
        ])))
        .unwrap_err();
        assert!(err.to_string().contains(RFL_TUI_READY_DELAY_MS));
    }
}
