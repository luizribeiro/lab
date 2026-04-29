use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,
    pub sandbox: SandboxConfig,
    pub filesystem: FilesystemConfig,
    pub limits: LimitsConfig,
    pub env: EnvConfig,
    pub darwin: DarwinConfig,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct SandboxConfig {
    pub allow_kvm: bool,
    pub allow_interactive_tty: bool,
    pub allow_non_pie_exec: bool,
    pub network: NetworkConfig,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct NetworkConfig {
    pub mode: NetworkConfigMode,
    /// Host allowlist consulted when `mode = "proxy"`. Ignored
    /// otherwise. Entries are host-pattern strings
    /// (`"api.example.com"`, `"*.cdn.example.com"`) as parsed by
    /// `outpost::DomainPattern`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub allow_hosts: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum NetworkConfigMode {
    #[default]
    Deny,
    AllowAll,
    Proxy,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct FilesystemConfig {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub read_paths: Vec<PathBuf>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub read_dirs: Vec<PathBuf>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub write_paths: Vec<PathBuf>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub write_dirs: Vec<PathBuf>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exec_paths: Vec<PathBuf>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exec_dirs: Vec<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct EnvConfig {
    pub inherit: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pass: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub block: Vec<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub set: BTreeMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct DarwinConfig {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub raw_seatbelt_rules: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct LimitsConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_open_files: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_address_space: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_cpu_time: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_processes: Option<u64>,
    pub disable_core_dumps: bool,
}

/// Loads the TOML config and returns it together with the absolute
/// path of the directory containing the config file. Pass that
/// directory to [`apply_config`] and [`resolve_command`] so relative
/// paths in the TOML resolve against the config file's location
/// rather than the caller's CWD.
pub fn load_config(path: &Path) -> Result<(Config, PathBuf)> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config file: {}", path.display()))?;
    let config: Config = toml::from_str(&contents)
        .with_context(|| format!("failed to parse config file: {}", path.display()))?;
    if matches!(&config.command, Some(v) if v.is_empty()) {
        anyhow::bail!("'command' must not be empty (omit it to use CLI args)");
    }
    let config_dir = std::path::absolute(path)
        .with_context(|| format!("failed to resolve config path: {}", path.display()))?
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("/"));
    Ok((config, config_dir))
}

/// How the child's network should be enforced, resolved from the TOML
/// config. `Proxy` carries a compiled `outpost::NetworkPolicy` that
/// the caller must spawn an `outpost-proxy` with before launching the
/// sandbox; the resulting loopback port is then handed to
/// `SandboxBuilder::network_proxy`.
#[derive(Debug)]
pub enum NetworkPlan {
    Deny,
    AllowAll,
    Proxy { policy: outpost::NetworkPolicy },
}

/// Resolves the [`NetworkPlan`] from the user's `[sandbox.network]`
/// configuration.
pub fn resolve_network_plan(config: &Config) -> Result<NetworkPlan> {
    let NetworkConfig { mode, allow_hosts } = &config.sandbox.network;
    match mode {
        NetworkConfigMode::Deny => {
            ensure_no_allow_hosts(allow_hosts, "deny")?;
            Ok(NetworkPlan::Deny)
        }
        NetworkConfigMode::AllowAll => {
            ensure_no_allow_hosts(allow_hosts, "allow_all")?;
            Ok(NetworkPlan::AllowAll)
        }
        NetworkConfigMode::Proxy => {
            let policy =
                outpost::NetworkPolicy::from_allowed_hosts(allow_hosts.iter().map(String::as_str))
                    .context("failed to parse [sandbox.network].allow_hosts")?;
            Ok(NetworkPlan::Proxy { policy })
        }
    }
}

fn ensure_no_allow_hosts(allow_hosts: &[String], mode: &str) -> Result<()> {
    if !allow_hosts.is_empty() {
        anyhow::bail!(
            "[sandbox.network]: allow_hosts has no effect when mode = \"{mode}\"; \
             set mode = \"proxy\" to enforce it"
        );
    }
    Ok(())
}

pub fn apply_config(config: &Config, config_dir: Option<&Path>) -> Result<lockin::SandboxBuilder> {
    let mut builder = lockin::Sandbox::builder()
        .allow_kvm(config.sandbox.allow_kvm)
        .allow_interactive_tty(config.sandbox.allow_interactive_tty)
        .allow_non_pie_exec(config.sandbox.allow_non_pie_exec);

    for p in &config.filesystem.read_paths {
        builder = builder.read_path(resolve_path(p, config_dir)?);
    }
    for p in &config.filesystem.read_dirs {
        builder = builder.read_dir(resolve_path(p, config_dir)?);
    }
    for p in &config.filesystem.write_paths {
        builder = builder.write_path(resolve_path(p, config_dir)?);
    }
    for p in &config.filesystem.write_dirs {
        builder = builder.write_dir(resolve_path(p, config_dir)?);
    }
    for p in &config.filesystem.exec_paths {
        builder = builder.exec_path(resolve_path(p, config_dir)?);
    }
    for p in &config.filesystem.exec_dirs {
        builder = builder.exec_dir(resolve_path(p, config_dir)?);
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

    for rule in &config.darwin.raw_seatbelt_rules {
        builder = builder.raw_seatbelt_rule(rule);
    }

    Ok(builder)
}

pub fn resolve_command(
    config: &Config,
    cli_args: &[std::ffi::OsString],
    config_dir: Option<&Path>,
) -> Result<(PathBuf, Vec<std::ffi::OsString>)> {
    let program_from_config = config.command.is_some();
    let mut argv: Vec<std::ffi::OsString> = config
        .command
        .as_ref()
        .map(|cmd| cmd.iter().map(std::ffi::OsString::from).collect())
        .unwrap_or_default();
    argv.extend(cli_args.iter().cloned());

    anyhow::ensure!(!argv.is_empty(), "no command specified");

    let program_path = Path::new(&argv[0]);
    if program_path.parent() == Some(Path::new("")) {
        anyhow::bail!(
            "lockin requires an explicit executable path; \
             use /usr/bin/{name}, ./{name}, or set command = [\"/path/to/{name}\"] in lockin.toml. \
             Bare PATH lookup is intentionally not supported.",
            name = program_path.display()
        );
    }
    // CLI-supplied program paths stay CWD-relative — they're typed at
    // a shell prompt, not loaded from the config file.
    let program_base = if program_from_config {
        config_dir
    } else {
        None
    };
    let program = resolve_path(program_path, program_base)?;
    let args = argv[1..].to_vec();
    Ok((program, args))
}

pub fn resolve_path(path: &Path, base: Option<&Path>) -> Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let joined = match base {
        Some(base) => base.join(path),
        None => path.to_path_buf(),
    };
    std::path::absolute(&joined)
        .with_context(|| format!("failed to resolve path: {}", joined.display()))
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
        assert_eq!(config.sandbox.network.mode, NetworkConfigMode::Deny);
    }

    #[test]
    fn full_config() {
        let config = parse(
            r#"
            command = ["/usr/bin/python3", "-u"]

            [sandbox]
            allow_kvm = false
            allow_interactive_tty = true
            allow_non_pie_exec = true

            [sandbox.network]
            mode = "allow_all"

            [filesystem]
            read_paths = ["/etc/hosts"]
            read_dirs = ["/usr/share"]
            write_paths = ["/var/log/app.log"]
            write_dirs = ["./data"]
            exec_paths = ["/usr/bin/git"]
            exec_dirs = ["/usr/local/bin"]

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

            [darwin]
            raw_seatbelt_rules = [
                "(allow iokit-open (iokit-user-client-class \"AGXDeviceUserClient\"))",
            ]
            "#,
        )
        .unwrap();

        assert_eq!(
            config,
            Config {
                command: Some(vec!["/usr/bin/python3".into(), "-u".into()]),
                sandbox: SandboxConfig {
                    allow_kvm: false,
                    allow_interactive_tty: true,
                    allow_non_pie_exec: true,
                    network: NetworkConfig {
                        mode: NetworkConfigMode::AllowAll,
                        allow_hosts: vec![],
                    },
                },
                filesystem: FilesystemConfig {
                    read_paths: vec![PathBuf::from("/etc/hosts")],
                    read_dirs: vec![PathBuf::from("/usr/share")],
                    write_paths: vec![PathBuf::from("/var/log/app.log")],
                    write_dirs: vec![PathBuf::from("./data")],
                    exec_paths: vec![PathBuf::from("/usr/bin/git")],
                    exec_dirs: vec![PathBuf::from("/usr/local/bin")],
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
                darwin: DarwinConfig {
                    raw_seatbelt_rules: vec![
                        "(allow iokit-open (iokit-user-client-class \"AGXDeviceUserClient\"))"
                            .to_string(),
                    ],
                },
            }
        );
    }

    #[test]
    fn filesystem_exec_paths_and_dirs_parse() {
        let config = parse(
            r#"
            [filesystem]
            exec_paths = ["/usr/bin/git", "/bin/sh"]
            exec_dirs = ["/usr/local/bin"]
            "#,
        )
        .unwrap();
        assert_eq!(
            config.filesystem.exec_paths,
            vec![PathBuf::from("/usr/bin/git"), PathBuf::from("/bin/sh")],
        );
        assert_eq!(
            config.filesystem.exec_dirs,
            vec![PathBuf::from("/usr/local/bin")],
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
    fn load_config_returns_parent_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("lockin.toml");
        std::fs::write(&path, "").unwrap();
        let (_, dir) = load_config(&path).unwrap();
        assert_eq!(dir, tmp.path());
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
        let (prog, args) =
            resolve_command(&config, &[os("script.py"), os("--flag")], None).unwrap();
        assert_eq!(prog, PathBuf::from("/usr/bin/python3"));
        assert_eq!(args, vec![os("script.py"), os("--flag")]);
    }

    #[test]
    fn resolve_config_command_with_fixed_args() {
        let config = Config {
            command: Some(vec!["/usr/bin/python3".into(), "-u".into()]),
            ..Default::default()
        };
        let (prog, args) = resolve_command(&config, &[os("script.py")], None).unwrap();
        assert_eq!(prog, PathBuf::from("/usr/bin/python3"));
        assert_eq!(args, vec![os("-u"), os("script.py")]);
    }

    #[test]
    fn resolve_config_command_only() {
        let config = Config {
            command: Some(vec!["/usr/bin/python3".into(), "main.py".into()]),
            ..Default::default()
        };
        let (prog, args) = resolve_command(&config, &[], None).unwrap();
        assert_eq!(prog, PathBuf::from("/usr/bin/python3"));
        assert_eq!(args, vec![os("main.py")]);
    }

    #[test]
    fn resolve_cli_args_only() {
        let config = Config::default();
        let (prog, args) =
            resolve_command(&config, &[os("/usr/bin/myapp"), os("--flag")], None).unwrap();
        assert_eq!(prog, PathBuf::from("/usr/bin/myapp"));
        assert_eq!(args, vec![os("--flag")]);
    }

    #[test]
    fn resolve_no_command_errors() {
        let config = Config::default();
        let err = resolve_command(&config, &[], None).unwrap_err();
        assert!(err.to_string().contains("no command specified"));
    }

    #[test]
    fn resolve_bare_command_from_config_rejected() {
        let config = Config {
            command: Some(vec!["python3".into()]),
            ..Default::default()
        };
        let err = resolve_command(&config, &[os("script.py")], None).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("explicit executable path") && msg.contains("Bare PATH"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn resolve_bare_command_from_cli_rejected() {
        let config = Config::default();
        let err = resolve_command(&config, &[os("python3"), os("--help")], None).unwrap_err();
        assert!(err.to_string().contains("explicit executable path"));
    }

    #[test]
    fn resolve_relative_path() {
        let config = Config::default();
        let (prog, _) = resolve_command(&config, &[os("./myapp")], None).unwrap();
        assert!(prog.is_absolute());
    }

    #[test]
    fn cli_program_path_resolves_against_cwd_not_config_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let config = Config::default();
        let (prog, _) = resolve_command(&config, &[os("./myapp")], Some(tmp.path())).unwrap();
        let expected = std::path::absolute("./myapp").unwrap();
        assert_eq!(prog, expected);
    }

    #[test]
    fn config_program_path_resolves_against_config_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let config = Config {
            command: Some(vec!["./bin/tool".into()]),
            ..Default::default()
        };
        let (prog, _) = resolve_command(&config, &[], Some(tmp.path())).unwrap();
        assert_eq!(prog, tmp.path().join("bin/tool"));
    }

    #[test]
    fn apply_default_config_builds_sandbox() {
        let config = Config::default();
        let builder = apply_config(&config, None).unwrap();
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
    fn apply_config_threads_darwin_raw_seatbelt_rules_into_builder() {
        let rule = "(allow iokit-open (iokit-user-client-class \"AGXDeviceUserClient\"))";
        let config = Config {
            darwin: DarwinConfig {
                raw_seatbelt_rules: vec![rule.to_string()],
            },
            ..Default::default()
        };
        let builder = apply_config(&config, None).unwrap();
        let cmd = builder.command(Path::new("/bin/echo")).unwrap();
        if cfg!(target_os = "macos") {
            let args: Vec<String> = cmd
                .as_command()
                .get_args()
                .map(|a| a.to_string_lossy().to_string())
                .collect();
            let joined = args.join(" ");
            assert!(
                joined.contains(rule),
                "raw seatbelt rule not threaded into sandbox-exec args: {joined}"
            );
        }
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
        assert!(apply_config(&config, None).is_ok());
    }

    #[test]
    fn resolve_path_joins_relative_against_base() {
        let base = Path::new("/etc/lockin");
        let resolved = resolve_path(Path::new("./data"), Some(base)).unwrap();
        assert_eq!(resolved, PathBuf::from("/etc/lockin/data"));
    }

    #[test]
    fn resolve_path_absolute_passes_through_with_base() {
        let base = Path::new("/etc/lockin");
        let resolved = resolve_path(Path::new("/var/log/app.log"), Some(base)).unwrap();
        assert_eq!(resolved, PathBuf::from("/var/log/app.log"));
    }

    #[test]
    fn resolve_path_no_base_falls_back_to_cwd() {
        let resolved = resolve_path(Path::new("./data"), None).unwrap();
        assert_eq!(resolved, std::path::absolute("./data").unwrap());
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn apply_config_threads_config_dir_through_every_filesystem_category() {
        let tmp = tempfile::tempdir().unwrap();
        let config = Config {
            filesystem: FilesystemConfig {
                read_paths: vec![PathBuf::from("./r-path")],
                read_dirs: vec![PathBuf::from("./r-dir")],
                write_paths: vec![PathBuf::from("./w-path")],
                write_dirs: vec![PathBuf::from("./w-dir")],
                exec_paths: vec![PathBuf::from("./e-path")],
                exec_dirs: vec![PathBuf::from("./e-dir")],
            },
            ..Default::default()
        };
        let builder = apply_config(&config, Some(tmp.path())).unwrap();
        let cmd = builder.command(Path::new("/bin/echo")).unwrap();
        let joined: String = cmd
            .as_command()
            .get_args()
            .map(|a| a.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(" ");
        for leaf in ["r-path", "r-dir", "w-path", "w-dir", "e-path", "e-dir"] {
            let expected = tmp.path().join(leaf);
            assert!(
                joined.contains(expected.to_str().unwrap()),
                "{leaf} not resolved against config dir in: {joined}",
            );
        }
    }

    #[test]
    fn load_config_relative_paths_resolve_against_config_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let config_path = tmp.path().join("lockin.toml");
        std::fs::write(
            &config_path,
            r#"
            [filesystem]
            read_dirs = ["./data"]
            write_paths = ["/var/log/app.log"]
            "#,
        )
        .unwrap();

        let (config, dir) = load_config(&config_path).unwrap();
        let resolved_read = resolve_path(&config.filesystem.read_dirs[0], Some(&dir)).unwrap();
        let resolved_write = resolve_path(&config.filesystem.write_paths[0], Some(&dir)).unwrap();

        assert_eq!(resolved_read, tmp.path().join("data"));
        assert_eq!(resolved_write, PathBuf::from("/var/log/app.log"));
        assert!(
            !resolved_read.starts_with(std::env::current_dir().unwrap()),
            "config-dir-relative path leaked CWD: {resolved_read:?}",
        );
    }

    #[test]
    fn load_missing_file_errors() {
        let err = load_config(Path::new("/nonexistent/lockin.toml")).unwrap_err();
        assert!(err.to_string().contains("failed to read config file"));
    }

    #[test]
    fn network_section_parses_proxy_mode_with_allow_hosts() {
        let config = parse(
            r#"
            [sandbox.network]
            mode = "proxy"
            allow_hosts = ["api.example.com", "*.cdn.example.com"]
            "#,
        )
        .unwrap();

        assert_eq!(config.sandbox.network.mode, NetworkConfigMode::Proxy);
        assert_eq!(
            config.sandbox.network.allow_hosts,
            vec![
                "api.example.com".to_string(),
                "*.cdn.example.com".to_string()
            ]
        );
    }

    #[test]
    fn network_mode_defaults_to_deny_when_section_absent() {
        let config = parse("").unwrap();
        assert_eq!(config.sandbox.network.mode, NetworkConfigMode::Deny);
        assert!(config.sandbox.network.allow_hosts.is_empty());
    }

    #[test]
    fn resolve_network_plan_defaults_to_deny() {
        let plan = resolve_network_plan(&Config::default()).unwrap();
        assert!(matches!(plan, NetworkPlan::Deny));
    }

    #[test]
    fn resolve_network_plan_compiles_proxy_allow_hosts_into_policy() {
        let config = Config {
            sandbox: SandboxConfig {
                network: NetworkConfig {
                    mode: NetworkConfigMode::Proxy,
                    allow_hosts: vec!["huggingface.co".into(), "*.hf.co".into()],
                },
                ..Default::default()
            },
            ..Default::default()
        };
        let plan = resolve_network_plan(&config).unwrap();
        let NetworkPlan::Proxy { policy } = plan else {
            panic!("expected Proxy plan, got Deny/AllowAll");
        };
        assert_eq!(
            policy.matches_host("huggingface.co"),
            outpost::PolicyAction::Allow
        );
        assert_eq!(
            policy.matches_host("api.hf.co"),
            outpost::PolicyAction::Allow
        );
        assert_eq!(policy.matches_host("evil.com"), outpost::PolicyAction::Deny);
    }

    #[test]
    fn resolve_network_plan_rejects_allow_hosts_with_non_proxy_mode() {
        for mode in [NetworkConfigMode::Deny, NetworkConfigMode::AllowAll] {
            let config = Config {
                sandbox: SandboxConfig {
                    network: NetworkConfig {
                        mode,
                        allow_hosts: vec!["example.com".into()],
                    },
                    ..Default::default()
                },
                ..Default::default()
            };
            let err = resolve_network_plan(&config).unwrap_err();
            assert!(
                err.to_string().contains("allow_hosts has no effect"),
                "expected inconsistent-config error for mode {mode:?}, got: {err}"
            );
        }
    }

    #[test]
    fn config_round_trips_through_toml() {
        let original = Config {
            command: Some(vec!["/usr/bin/python3".into(), "-u".into()]),
            sandbox: SandboxConfig {
                allow_kvm: false,
                allow_interactive_tty: true,
                allow_non_pie_exec: false,
                network: NetworkConfig {
                    mode: NetworkConfigMode::Proxy,
                    allow_hosts: vec!["api.example.com".into(), "*.cdn.example.com".into()],
                },
            },
            filesystem: FilesystemConfig {
                read_paths: vec![PathBuf::from("/etc/hosts")],
                write_dirs: vec![PathBuf::from("./data")],
                ..Default::default()
            },
            limits: LimitsConfig {
                max_open_files: Some(1024),
                disable_core_dumps: true,
                ..Default::default()
            },
            env: EnvConfig {
                inherit: false,
                pass: vec!["PATH".into()],
                set: BTreeMap::from([("LANG".into(), "C.UTF-8".into())]),
                block: vec!["AWS_*".into()],
            },
            darwin: DarwinConfig {
                raw_seatbelt_rules: vec!["(allow default)".into()],
            },
        };

        let serialized = toml::to_string(&original).expect("serialize");
        let deserialized: Config = toml::from_str(&serialized)
            .unwrap_or_else(|e| panic!("failed to round-trip: {e}\n---\n{serialized}"));
        assert_eq!(deserialized, original, "round-trip mismatch:\n{serialized}");
    }

    #[test]
    fn default_config_serializes_without_optional_fields() {
        let toml_str = toml::to_string(&Config::default()).unwrap();
        assert!(
            !toml_str.contains("command"),
            "default emits command: {toml_str}"
        );
        assert!(
            !toml_str.contains("allow_hosts"),
            "default emits allow_hosts: {toml_str}"
        );
        assert!(
            !toml_str.contains("max_open_files"),
            "default emits max_open_files: {toml_str}"
        );
    }

    #[test]
    fn resolve_network_plan_rejects_invalid_host_patterns() {
        let config = Config {
            sandbox: SandboxConfig {
                network: NetworkConfig {
                    mode: NetworkConfigMode::Proxy,
                    allow_hosts: vec!["bad..host".into()],
                },
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(resolve_network_plan(&config).is_err());
    }
}
