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
    pub darwin: DarwinConfig,
}

#[derive(Debug, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct SandboxConfig {
    pub allow_kvm: bool,
    pub allow_interactive_tty: bool,
    pub allow_non_pie_exec: bool,
    pub network: NetworkConfig,
}

#[derive(Debug, Deserialize, Default, PartialEq, Clone)]
#[serde(default, deny_unknown_fields)]
pub struct NetworkConfig {
    pub mode: NetworkConfigMode,
    /// Host allowlist consulted when `mode = "proxy"`. Ignored
    /// otherwise. Entries are host-pattern strings
    /// (`"api.example.com"`, `"*.cdn.example.com"`) as parsed by
    /// `outpost::DomainPattern`.
    pub allow_hosts: Vec<String>,
}

#[derive(Debug, Deserialize, Default, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum NetworkConfigMode {
    #[default]
    Deny,
    AllowAll,
    Proxy,
}

#[derive(Debug, Deserialize, Default, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct FilesystemConfig {
    pub read_paths: Vec<PathBuf>,
    pub read_dirs: Vec<PathBuf>,
    pub write_paths: Vec<PathBuf>,
    pub write_dirs: Vec<PathBuf>,
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
pub struct DarwinConfig {
    pub raw_seatbelt_rules: Vec<String>,
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

pub fn apply_config(config: &Config) -> Result<lockin::SandboxBuilder> {
    let mut builder = lockin::Sandbox::builder()
        .allow_kvm(config.sandbox.allow_kvm)
        .allow_interactive_tty(config.sandbox.allow_interactive_tty)
        .allow_non_pie_exec(config.sandbox.allow_non_pie_exec);

    builder = builder.library_paths_from_env();
    for dir in &config.filesystem.library_paths {
        builder = builder.library_path(resolve_path(dir)?);
    }

    for p in &config.filesystem.read_paths {
        builder = builder.read_path(resolve_path(p)?);
    }
    for p in &config.filesystem.read_dirs {
        builder = builder.read_dir(resolve_path(p)?);
    }
    for p in &config.filesystem.write_paths {
        builder = builder.write_path(resolve_path(p)?);
    }
    for p in &config.filesystem.write_dirs {
        builder = builder.write_dir(resolve_path(p)?);
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

    for rule in &config.darwin.raw_seatbelt_rules {
        builder = builder.raw_seatbelt_rule(rule);
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
    if program_path.parent() == Some(Path::new("")) {
        anyhow::bail!(
            "lockin requires an explicit executable path; \
             use /usr/bin/{name}, ./{name}, or set command = [\"/path/to/{name}\"] in lockin.toml. \
             Bare PATH lookup is intentionally not supported.",
            name = program_path.display()
        );
    }
    let program = resolve_path(program_path)?;
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
    fn resolve_bare_command_from_config_rejected() {
        let config = Config {
            command: Some(vec!["python3".into()]),
            ..Default::default()
        };
        let err = resolve_command(&config, &[os("script.py")]).unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("explicit executable path") && msg.contains("Bare PATH"),
            "unexpected error: {msg}"
        );
    }

    #[test]
    fn resolve_bare_command_from_cli_rejected() {
        let config = Config::default();
        let err = resolve_command(&config, &[os("python3"), os("--help")]).unwrap_err();
        assert!(err.to_string().contains("explicit executable path"));
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
    fn apply_config_threads_darwin_raw_seatbelt_rules_into_builder() {
        let rule = "(allow iokit-open (iokit-user-client-class \"AGXDeviceUserClient\"))";
        let config = Config {
            darwin: DarwinConfig {
                raw_seatbelt_rules: vec![rule.to_string()],
            },
            ..Default::default()
        };
        let builder = apply_config(&config).unwrap();
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
        assert!(apply_config(&config).is_ok());
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
