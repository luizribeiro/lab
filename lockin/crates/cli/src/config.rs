use std::collections::BTreeMap;
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
    pub env: EnvConfig,
}

#[derive(Debug, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct SandboxConfig {
    pub allow_network: bool,
    pub allow_kvm: bool,
    pub allow_interactive_tty: bool,
    pub allow_non_pie_exec: bool,
}

#[derive(Debug, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct FilesystemConfig {
    pub read_only_paths: Vec<PathBuf>,
    pub read_only_dirs: Vec<PathBuf>,
    pub read_write_paths: Vec<PathBuf>,
    pub read_write_dirs: Vec<PathBuf>,
    pub ioctl_paths: Vec<PathBuf>,
    pub ioctl_dirs: Vec<PathBuf>,
    pub library_paths: Vec<PathBuf>,
}

#[derive(Debug, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct EnvConfig {
    pub inherit: bool,
    pub pass: Vec<String>,
    pub set: BTreeMap<String, String>,
    pub block: Vec<String>,
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

pub fn apply_config(config: &Config) -> Result<lockin::SandboxBuilder> {
    let mut builder = lockin::Sandbox::builder()
        .allow_network(config.sandbox.allow_network)
        .allow_kvm(config.sandbox.allow_kvm)
        .allow_interactive_tty(config.sandbox.allow_interactive_tty)
        .allow_non_pie_exec(config.sandbox.allow_non_pie_exec);

    builder = builder.library_paths_from_env();
    for dir in &config.filesystem.library_paths {
        builder = builder.library_path(resolve_path(dir)?);
    }

    for p in &config.filesystem.read_only_paths {
        builder = builder.read_only_path(resolve_path(p)?);
    }
    for p in &config.filesystem.read_only_dirs {
        builder = builder.read_only_dir(resolve_path(p)?);
    }
    for p in &config.filesystem.read_write_paths {
        builder = builder.read_write_path(resolve_path(p)?);
    }
    for p in &config.filesystem.read_write_dirs {
        builder = builder.read_write_dir(resolve_path(p)?);
    }
    for p in &config.filesystem.ioctl_paths {
        builder = builder.ioctl_path(resolve_path(p)?);
    }
    for p in &config.filesystem.ioctl_dirs {
        builder = builder.ioctl_dir(resolve_path(p)?);
    }

    if let Some(n) = config.limits.max_open_files {
        builder = builder.max_open_files(n);
    }
    if let Some(n) = config.limits.max_address_space {
        builder = builder.max_address_space(n);
    }
    if let Some(n) = config.limits.max_cpu_time {
        builder = builder.max_cpu_time(n);
    }
    if let Some(n) = config.limits.max_processes {
        builder = builder.max_processes(n);
    }
    if config.limits.disable_core_dumps {
        builder = builder.disable_core_dumps();
    }

    Ok(builder)
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
            allow_non_pie_exec = true

            [filesystem]
            read_only_paths = ["/etc/hosts"]
            read_only_dirs = ["/usr/share"]
            read_write_paths = ["/var/log/app.log"]
            read_write_dirs = ["./data"]
            ioctl_paths = ["/dev/net/tun"]
            ioctl_dirs = []
            library_paths = ["/usr/lib"]

            [limits]
            max_open_files = 1024
            max_address_space = 4294967296
            max_cpu_time = 60
            max_processes = 100
            disable_core_dumps = true

            [env]
            inherit = false
            pass = ["PATH", "HOME"]
            set = { LANG = "C.UTF-8" }
            block = ["AWS_*", "GITHUB_TOKEN"]
            "#,
        )
        .unwrap();

        assert_eq!(
            config,
            Config {
                command: Some(vec!["/usr/bin/python3".into(), "-u".into()]),
                sandbox: SandboxConfig {
                    allow_network: true,
                    allow_kvm: false,
                    allow_interactive_tty: true,
                    allow_non_pie_exec: true,
                },
                filesystem: FilesystemConfig {
                    read_only_paths: vec![PathBuf::from("/etc/hosts")],
                    read_only_dirs: vec![PathBuf::from("/usr/share")],
                    read_write_paths: vec![PathBuf::from("/var/log/app.log")],
                    read_write_dirs: vec![PathBuf::from("./data")],
                    ioctl_paths: vec![PathBuf::from("/dev/net/tun")],
                    ioctl_dirs: vec![],
                    library_paths: vec![PathBuf::from("/usr/lib")],
                },
                limits: LimitsConfig {
                    max_open_files: Some(1024),
                    max_address_space: Some(4294967296),
                    max_cpu_time: Some(60),
                    max_processes: Some(100),
                    disable_core_dumps: true,
                },
                env: EnvConfig {
                    inherit: false,
                    pass: vec!["PATH".to_string(), "HOME".to_string()],
                    set: [("LANG".to_string(), "C.UTF-8".to_string())].into(),
                    block: vec!["AWS_*".to_string(), "GITHUB_TOKEN".to_string()],
                },
            }
        );
    }

    #[test]
    fn unknown_field_rejected() {
        let err = parse("[sandbox]\ntypo_field = true").unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn env_defaults() {
        let config = parse("").unwrap();
        assert!(!config.env.inherit);
        assert!(config.env.pass.is_empty());
        assert!(config.env.set.is_empty());
        assert!(config.env.block.is_empty());
    }

    #[test]
    fn env_section_parses() {
        let config = parse(
            r#"
            [env]
            inherit = false
            pass = ["PATH"]
            set = { TERM = "xterm-256color" }
            block = ["AWS_*"]
            "#,
        )
        .unwrap();
        assert!(!config.env.inherit);
        assert_eq!(config.env.pass, vec!["PATH".to_string()]);
        assert_eq!(
            config.env.set,
            BTreeMap::from([("TERM".to_string(), "xterm-256color".to_string())]),
        );
        assert_eq!(config.env.block, vec!["AWS_*".to_string()]);
    }

    #[test]
    fn env_set_accepts_explicit_table_syntax() {
        let config = parse(
            r#"
            [env.set]
            LANG = "C.UTF-8"
            TERM = "xterm-256color"
            "#,
        )
        .unwrap();
        assert_eq!(
            config.env.set,
            BTreeMap::from([
                ("LANG".to_string(), "C.UTF-8".to_string()),
                ("TERM".to_string(), "xterm-256color".to_string()),
            ]),
        );
    }

    #[test]
    fn env_unknown_field_rejected() {
        let err = parse("[env]\ngarbage = 1").unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn library_paths_from_env_field_rejected() {
        let err = parse("[filesystem]\nlibrary_paths_from_env = true").unwrap_err();
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
    fn apply_default_config_builds_sandbox() {
        let config = Config::default();
        let builder = apply_config(&config).unwrap();
        let cmd = builder.command(Path::new("/bin/echo")).unwrap();
        let program = cmd.as_command().get_program().to_string_lossy().to_string();
        let expected = if cfg!(target_os = "macos") {
            "sandbox-exec"
        } else {
            "syd"
        };
        assert!(
            program.contains(expected),
            "expected {expected} in program, got: {program}"
        );
    }

    #[test]
    fn apply_config_with_limits() {
        let config = Config {
            limits: LimitsConfig {
                max_open_files: Some(64),
                disable_core_dumps: true,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(apply_config(&config).is_ok());
    }

    #[test]
    fn load_missing_file_errors() {
        let err = load_config(Path::new("/nonexistent/lockin.toml")).unwrap_err();
        assert!(err.to_string().contains("failed to read config file"));
    }
}
