use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub command: Option<Vec<String>>,
    pub sandbox: SandboxConfig,
    pub filesystem: FilesystemConfig,
    pub limits: LimitsConfig,
}

#[derive(Debug, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct SandboxConfig {
    pub allow_network: bool,
    pub allow_kvm: bool,
    pub allow_interactive_tty: bool,
    pub syd_path: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct FilesystemConfig {
    pub read_only: Vec<PathBuf>,
    pub read_only_dirs: Vec<PathBuf>,
    pub read_write: Vec<PathBuf>,
    pub read_write_dirs: Vec<PathBuf>,
    pub ioctl: Vec<PathBuf>,
    pub ioctl_dirs: Vec<PathBuf>,
    pub library_dirs: Vec<PathBuf>,
    pub library_dirs_from_env: bool,
}

#[derive(Debug, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct LimitsConfig {
    pub max_open_files: Option<u64>,
    pub max_address_space: Option<u64>,
    pub max_cpu_time: Option<u64>,
    pub max_processes: Option<u64>,
    pub disable_core_dumps: bool,
}

pub fn load_config(path: &Path) -> Result<Config> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config file: {}", path.display()))?;
    let config: Config = toml::from_str(&contents)
        .with_context(|| format!("failed to parse config file: {}", path.display()))?;
    if matches!(&config.command, Some(v) if v.is_empty()) {
        anyhow::bail!("'command' must not be empty (omit it to use CLI args)");
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(toml: &str) -> Result<Config, toml::de::Error> {
        toml::from_str(toml)
    }

    #[test]
    fn empty_config() {
        let config = parse("").unwrap();
        assert_eq!(config, Config::default());
    }

    #[test]
    fn minimal_sandbox_section() {
        let config = parse("[sandbox]").unwrap();
        assert!(!config.sandbox.allow_network);
    }

    #[test]
    fn full_config() {
        let config = parse(
            r#"
            command = ["/usr/bin/python3", "-u"]

            [sandbox]
            allow_network = true
            allow_kvm = false
            allow_interactive_tty = true
            syd_path = "/usr/bin/syd"

            [filesystem]
            read_only = ["/etc/hosts"]
            read_only_dirs = ["/usr/share"]
            read_write = ["/var/log/app.log"]
            read_write_dirs = ["./data"]
            ioctl = ["/dev/net/tun"]
            ioctl_dirs = []
            library_dirs = ["/usr/lib"]
            library_dirs_from_env = true

            [limits]
            max_open_files = 1024
            max_address_space = 4294967296
            max_cpu_time = 60
            max_processes = 100
            disable_core_dumps = true
            "#,
        )
        .unwrap();

        assert_eq!(
            config.command,
            Some(vec!["/usr/bin/python3".into(), "-u".into()])
        );
        assert!(config.sandbox.allow_network);
        assert!(config.sandbox.allow_interactive_tty);
        assert_eq!(config.sandbox.syd_path, Some(PathBuf::from("/usr/bin/syd")));
        assert_eq!(
            config.filesystem.read_only,
            vec![PathBuf::from("/etc/hosts")]
        );
        assert_eq!(
            config.filesystem.library_dirs,
            vec![PathBuf::from("/usr/lib")]
        );
        assert!(config.filesystem.library_dirs_from_env);
        assert_eq!(config.limits.max_open_files, Some(1024));
        assert_eq!(config.limits.max_address_space, Some(4294967296));
        assert!(config.limits.disable_core_dumps);
    }

    #[test]
    fn unknown_field_rejected() {
        let err = parse("[sandbox]\ntypo_field = true").unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn unknown_top_level_field_rejected() {
        let err = parse("bogus = 123").unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn command_field_absent() {
        let config = parse("[sandbox]").unwrap();
        assert!(config.command.is_none());
    }

    #[test]
    fn command_field_empty_array_rejected() {
        let config: Config = parse("command = []").unwrap();
        assert_eq!(config.command, Some(vec![]));

        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), "command = []").unwrap();
        let err = load_config(tmp.path()).unwrap_err();
        assert!(err.to_string().contains("must not be empty"));
    }

    #[test]
    fn load_missing_file_errors() {
        let err = load_config(Path::new("/nonexistent/lockin.toml")).unwrap_err();
        assert!(err.to_string().contains("failed to read config file"));
    }
}
