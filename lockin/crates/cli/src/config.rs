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

pub fn resolve_command(
    config: &Config,
    cli_args: &[std::ffi::OsString],
) -> Result<(PathBuf, Vec<std::ffi::OsString>)> {
    let mut argv: Vec<std::ffi::OsString> = config
        .command
        .as_ref()
        .map(|cmd| cmd.iter().map(std::ffi::OsString::from).collect())
        .unwrap_or_default();
    argv.extend(cli_args.iter().cloned());

    anyhow::ensure!(!argv.is_empty(), "no command specified");

    let program_path = Path::new(&argv[0]);
    let program = if program_path.parent() == Some(Path::new("")) {
        program_path.to_path_buf()
    } else {
        resolve_path(program_path)?
    };
    let args = argv[1..].to_vec();
    Ok((program, args))
}

pub fn resolve_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::path::absolute(path)
            .with_context(|| format!("failed to resolve path: {}", path.display()))
    }
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

    fn os(s: &str) -> std::ffi::OsString {
        s.into()
    }

    #[test]
    fn resolve_config_command_plus_cli_args() {
        let config = Config {
            command: Some(vec!["/usr/bin/python3".into()]),
            ..Default::default()
        };
        let (prog, args) = resolve_command(&config, &[os("script.py"), os("--flag")]).unwrap();
        assert_eq!(prog, PathBuf::from("/usr/bin/python3"));
        assert_eq!(args, vec![os("script.py"), os("--flag")]);
    }

    #[test]
    fn resolve_config_command_with_fixed_args() {
        let config = Config {
            command: Some(vec!["/usr/bin/python3".into(), "-u".into()]),
            ..Default::default()
        };
        let (prog, args) = resolve_command(&config, &[os("script.py")]).unwrap();
        assert_eq!(prog, PathBuf::from("/usr/bin/python3"));
        assert_eq!(args, vec![os("-u"), os("script.py")]);
    }

    #[test]
    fn resolve_config_command_only() {
        let config = Config {
            command: Some(vec!["/usr/bin/python3".into(), "main.py".into()]),
            ..Default::default()
        };
        let (prog, args) = resolve_command(&config, &[]).unwrap();
        assert_eq!(prog, PathBuf::from("/usr/bin/python3"));
        assert_eq!(args, vec![os("main.py")]);
    }

    #[test]
    fn resolve_cli_args_only() {
        let config = Config::default();
        let (prog, args) = resolve_command(&config, &[os("/usr/bin/myapp"), os("--flag")]).unwrap();
        assert_eq!(prog, PathBuf::from("/usr/bin/myapp"));
        assert_eq!(args, vec![os("--flag")]);
    }

    #[test]
    fn resolve_no_command_errors() {
        let config = Config::default();
        let err = resolve_command(&config, &[]).unwrap_err();
        assert!(err.to_string().contains("no command specified"));
    }

    #[test]
    fn resolve_bare_command_passes_through() {
        let config = Config {
            command: Some(vec!["python3".into()]),
            ..Default::default()
        };
        let (prog, _) = resolve_command(&config, &[os("script.py")]).unwrap();
        assert_eq!(prog, PathBuf::from("python3"));
    }

    #[test]
    fn resolve_relative_path() {
        let config = Config::default();
        let (prog, _) = resolve_command(&config, &[os("./myapp")]).unwrap();
        assert!(prog.is_absolute());
    }

    #[test]
    fn load_missing_file_errors() {
        let err = load_config(Path::new("/nonexistent/lockin.toml")).unwrap_err();
        assert!(err.to_string().contains("failed to read config file"));
    }
}
