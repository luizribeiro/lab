use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const DEFAULT_SEARCH_PROVIDER: &str = "duckduckgo";
const DEFAULT_TIMEOUT_SECS: u64 = 20;
const DEFAULT_MAX_BODY_BYTES: u64 = 5_000_000;
const DEFAULT_USER_AGENT: &str = "scope/0.1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default = "default_search_provider")]
    pub default_search_provider: String,
    #[serde(default)]
    pub http: HttpConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HttpConfig {
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default = "default_max_body_bytes")]
    pub max_body_bytes: u64,
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
}

fn default_search_provider() -> String {
    DEFAULT_SEARCH_PROVIDER.to_string()
}

fn default_timeout_secs() -> u64 {
    DEFAULT_TIMEOUT_SECS
}

fn default_max_body_bytes() -> u64 {
    DEFAULT_MAX_BODY_BYTES
}

fn default_user_agent() -> String {
    DEFAULT_USER_AGENT.to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            default_search_provider: default_search_provider(),
            http: HttpConfig::default(),
        }
    }
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            timeout_secs: default_timeout_secs(),
            max_body_bytes: default_max_body_bytes(),
            user_agent: default_user_agent(),
        }
    }
}

impl Config {
    pub fn load(explicit: Option<&Path>) -> Result<Self> {
        if let Some(path) = explicit {
            return Self::read_file(path);
        }
        match discover_config_path() {
            Some(path) if path.exists() => Self::read_file(&path),
            _ => Ok(Self::default()),
        }
    }

    fn read_file(path: &Path) -> Result<Self> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("reading config file {}", path.display()))?;
        toml::from_str(&contents)
            .with_context(|| format!("parsing config file {}", path.display()))
    }
}

fn discover_config_path() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("scope").join("config.toml"));
        }
    }
    std::env::var_os("HOME")
        .map(|home| PathBuf::from(home).join(".config").join("scope").join("config.toml"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use tempfile::TempDir;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        key: &'static str,
        previous: Option<std::ffi::OsString>,
    }

    impl EnvGuard {
        fn set(key: &'static str, value: &Path) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, previous }
        }

        fn unset(key: &'static str) -> Self {
            let previous = std::env::var_os(key);
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    #[test]
    fn defaults_match_expected() {
        let config = Config::default();
        assert_eq!(config.default_search_provider, "duckduckgo");
        assert_eq!(config.http.timeout_secs, 20);
        assert_eq!(config.http.max_body_bytes, 5_000_000);
        assert_eq!(config.http.user_agent, "scope/0.1");
    }

    #[test]
    fn parses_valid_toml() {
        let toml = r#"
default_search_provider = "kagi"

[http]
timeout_secs = 30
max_body_bytes = 1000
user_agent = "test/1.0"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.default_search_provider, "kagi");
        assert_eq!(config.http.timeout_secs, 30);
        assert_eq!(config.http.max_body_bytes, 1000);
        assert_eq!(config.http.user_agent, "test/1.0");
    }

    #[test]
    fn parses_partial_toml_with_defaults() {
        let config: Config = toml::from_str(r#"default_search_provider = "kagi""#).unwrap();
        assert_eq!(config.default_search_provider, "kagi");
        assert_eq!(config.http, HttpConfig::default());
    }

    #[test]
    fn rejects_unknown_top_level_field() {
        let err = toml::from_str::<Config>(r#"unknown_field = true"#).unwrap_err();
        assert!(err.to_string().contains("unknown"));
    }

    #[test]
    fn rejects_unknown_http_field() {
        let toml = r#"
[http]
unknown = 1
"#;
        let err = toml::from_str::<Config>(toml).unwrap_err();
        assert!(err.to_string().contains("unknown"));
    }

    #[test]
    fn explicit_path_wins_over_xdg() {
        let _lock = ENV_LOCK.lock().unwrap();
        let xdg_dir = TempDir::new().unwrap();
        std::fs::create_dir_all(xdg_dir.path().join("scope")).unwrap();
        std::fs::write(
            xdg_dir.path().join("scope").join("config.toml"),
            r#"default_search_provider = "from_xdg""#,
        )
        .unwrap();
        let _xdg = EnvGuard::set("XDG_CONFIG_HOME", xdg_dir.path());

        let explicit_dir = TempDir::new().unwrap();
        let explicit_path = explicit_dir.path().join("explicit.toml");
        std::fs::write(&explicit_path, r#"default_search_provider = "from_explicit""#).unwrap();

        let config = Config::load(Some(&explicit_path)).unwrap();
        assert_eq!(config.default_search_provider, "from_explicit");
    }

    #[test]
    fn xdg_path_used_when_env_set() {
        let _lock = ENV_LOCK.lock().unwrap();
        let xdg_dir = TempDir::new().unwrap();
        std::fs::create_dir_all(xdg_dir.path().join("scope")).unwrap();
        std::fs::write(
            xdg_dir.path().join("scope").join("config.toml"),
            r#"default_search_provider = "from_xdg""#,
        )
        .unwrap();
        let _xdg = EnvGuard::set("XDG_CONFIG_HOME", xdg_dir.path());

        let config = Config::load(None).unwrap();
        assert_eq!(config.default_search_provider, "from_xdg");
    }

    #[test]
    fn missing_file_returns_defaults() {
        let _lock = ENV_LOCK.lock().unwrap();
        let empty = TempDir::new().unwrap();
        let _xdg = EnvGuard::set("XDG_CONFIG_HOME", empty.path());
        let _home = EnvGuard::unset("HOME");

        let config = Config::load(None).unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn explicit_missing_file_errors() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("nope.toml");
        assert!(Config::load(Some(&missing)).is_err());
    }
}
