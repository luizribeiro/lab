//! Environment-variable parsing for the `rfl-tui` binary (scope §T2 step 1).

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};

pub const RFL_BUS_FD: &str = "RFL_BUS_FD";
pub const RFL_PROJECT_ROOT: &str = "RFL_PROJECT_ROOT";
pub const RFL_TUI_TEST_MODE: &str = "RFL_TUI_TEST_MODE";
pub const RFL_TUI_READY_DELAY_MS: &str = "RFL_TUI_READY_DELAY_MS";
pub const RFL_TUI_MAX_LIFETIME: &str = "RFL_TUI_MAX_LIFETIME";

#[derive(Debug, Clone)]
pub struct TuiEnv {
    pub bus_fd: i32,
    pub project_root: PathBuf,
    pub test_mode: bool,
    pub ready_delay_ms: Option<u64>,
    pub max_lifetime_secs: Option<u64>,
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

    Ok(TuiEnv {
        bus_fd,
        project_root,
        test_mode,
        ready_delay_ms,
        max_lifetime_secs,
    })
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
