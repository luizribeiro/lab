use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

mod glob;

/// Env vars whose presence in the parent process can compromise sandbox
/// integrity (preload-style hijacks). Always stripped, regardless of
/// `[env]` policy. Empty on platforms with no relevant variables.
pub const BUILTIN_ENV_BLOCKLIST: &[&str] = &[
    #[cfg(target_os = "linux")]
    "LD_PRELOAD",
    #[cfg(target_os = "linux")]
    "LD_LIBRARY_PATH",
    #[cfg(target_os = "linux")]
    "LD_AUDIT",
    #[cfg(target_os = "macos")]
    "DYLD_INSERT_LIBRARIES",
    #[cfg(target_os = "macos")]
    "DYLD_LIBRARY_PATH",
    #[cfg(target_os = "macos")]
    "DYLD_FRAMEWORK_PATH",
];

/// Env vars the sandbox library sets on the command itself (private
/// tmpdir wiring and observation-mode log routing). Preserved across
/// `env_clear` so `inherit = false` runs don't lose them.
pub const SANDBOX_OWNED_ENV: &[&str] = &[
    "TMPDIR",
    "TMP",
    "TEMP",
    "SYD_LOG",
    "SYD_LOG_FD",
    "SYD_NO_SYSLOG",
];

/// Mutates `cmd` so its env reflects `env_config` resolved against
/// `parent_env`. Mirrors what every lockin entry point needs (run mode,
/// trace mode, future modes), so it lives here rather than at any one
/// caller. Non-UTF-8 env keys are skipped in pass matching and block
/// filtering; glob matching is byte-level ASCII.
pub fn apply_env<I>(env_config: &EnvConfig, cmd: &mut lockin::SandboxedCommand, parent_env: I)
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let parent: Vec<(OsString, OsString)> = parent_env.into_iter().collect();
    let blocklist: Vec<&str> = BUILTIN_ENV_BLOCKLIST
        .iter()
        .copied()
        .chain(env_config.block.iter().map(String::as_str))
        .collect();
    let is_blocked = |name: &str| blocklist.iter().any(|p| glob::matches(p, name));

    if env_config.inherit {
        for (key, _) in &parent {
            if key.to_str().is_some_and(is_blocked) {
                cmd.env_remove(key);
            }
        }
    } else {
        let preserved: Vec<(OsString, OsString)> = cmd
            .as_command()
            .get_envs()
            .filter_map(|(k, v)| {
                let name = k.to_str()?;
                if SANDBOX_OWNED_ENV.contains(&name) {
                    v.map(|v| (k.to_owned(), v.to_owned()))
                } else {
                    None
                }
            })
            .collect();
        cmd.env_clear();
        for (k, v) in preserved {
            cmd.env(k, v);
        }
        for (key, value) in &parent {
            let Some(name) = key.to_str() else { continue };
            if env_config.pass.iter().any(|p| glob::matches(p, name)) && !is_blocked(name) {
                cmd.env(key, value);
            }
        }
    }

    for (key, value) in &env_config.set {
        if !is_blocked(key) {
            cmd.env(key, value);
        }
    }
}

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

/// Apply the user's `lockin.toml` policy onto an existing builder. Use
/// this when the caller has already configured non-policy aspects of
/// the builder (observation mode, fd inheritance) and just needs the
/// user policy layered on top.
pub fn apply_config_to_builder(
    builder: lockin::SandboxBuilder,
    config: &Config,
    config_dir: Option<&Path>,
) -> Result<lockin::SandboxBuilder> {
    let mut builder = builder
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

/// Apply the user's `lockin.toml` policy to a fresh builder. Equivalent
/// to `apply_config_to_builder(lockin::Sandbox::builder(), config, config_dir)`.
pub fn apply_config(config: &Config, config_dir: Option<&Path>) -> Result<lockin::SandboxBuilder> {
    apply_config_to_builder(lockin::Sandbox::builder(), config, config_dir)
}

pub struct EnforcedCommandSpec<'a> {
    /// The builder to apply policy onto. Callers running under
    /// observation pass a builder that already has observation/fd setup;
    /// run-mode callers pass `lockin::Sandbox::builder()`.
    pub builder: lockin::SandboxBuilder,
    pub config: &'a Config,
    pub config_dir: Option<&'a Path>,
    pub program: &'a Path,
    pub args: &'a [OsString],
    pub current_dir: Option<&'a Path>,
    pub network: lockin::NetworkMode,
    /// The parent's environment, typically `std::env::vars_os().collect()`.
    /// Used by `apply_env` to resolve inherit/pass/block.
    pub parent_env: Vec<(OsString, OsString)>,
    /// Env applied after the user's `[env]` policy. Used by the CLI to
    /// inject HTTP_PROXY/HTTPS_PROXY/etc. for proxy-mode runs. Passed
    /// through verbatim — not blocklist-filtered by lockin-config.
    pub extra_env: Vec<(OsString, OsString)>,
}

/// Builds a sandboxed command by applying the canonical enforced-run pipeline:
/// user policy onto the supplied builder, resolved network mode, command
/// construction, argv/current-dir, `[env]` policy, then `extra_env` overrides.
pub fn build_enforced_command(spec: EnforcedCommandSpec<'_>) -> Result<lockin::SandboxedCommand> {
    let builder =
        apply_config_to_builder(spec.builder, spec.config, spec.config_dir)?.network(spec.network);

    let mut cmd = builder.command(spec.program)?;

    cmd.args(spec.args);
    if let Some(dir) = spec.current_dir {
        cmd.current_dir(dir);
    }

    apply_env(&spec.config.env, &mut cmd, spec.parent_env);

    for (k, v) in spec.extra_env {
        cmd.env(k, v);
    }

    Ok(cmd)
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

/// Resolves a CLI-supplied program name to an absolute executable path.
///
/// Three forms:
/// - Already absolute → returned as-is.
/// - Contains a path separator (e.g. `./script`, `subdir/prog`) →
///   joined onto `current_dir` (or `std::env::current_dir()` if `None`)
///   and absolutized.
/// - Bare name (`echo`, `git`) → walks `PATH` for the first executable
///   regular file with the matching name.
///
/// Returns an error (never panics) if the bare-name lookup finds
/// nothing, so callers feeding the result into something that asserts
/// absoluteness (e.g. `SandboxBuilder::command`) can surface a clean
/// error to the user instead of crashing.
///
/// Distinct from [`resolve_command`], which intentionally rejects bare
/// PATH lookups for the run path (the seed config or CLI must name an
/// explicit path). `infer` mode has no seed config to back-fill the
/// program from, so bare names are accepted here and resolved via PATH.
pub fn resolve_executable(name: &std::ffi::OsStr, current_dir: Option<&Path>) -> Result<PathBuf> {
    let path = Path::new(name);
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    // Distinguishes `./foo` / `subdir/foo` (multi-component) from a bare `foo`.
    if path.components().count() > 1 {
        let base = match current_dir {
            Some(d) => d.to_path_buf(),
            None => std::env::current_dir().context("failed to read current working directory")?,
        };
        return std::path::absolute(base.join(path))
            .with_context(|| format!("failed to resolve relative program path: {name:?}"));
    }

    let path_var = std::env::var_os("PATH").ok_or_else(|| anyhow::anyhow!("PATH is not set"))?;
    for dir in std::env::split_paths(&path_var) {
        if !dir.is_absolute() {
            continue;
        }
        let candidate = dir.join(path);
        if is_executable_file(&candidate) {
            return Ok(candidate);
        }
    }
    anyhow::bail!("executable not found in PATH: {}", path.display())
}

fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    match path.metadata() {
        Ok(m) => m.is_file() && m.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
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

    mod resolve_executable_tests {
        use super::*;
        use std::os::unix::fs::PermissionsExt;
        use std::sync::Mutex;

        // env::set_var mutates global state; serialize PATH-touching tests.
        static ENV_LOCK: Mutex<()> = Mutex::new(());

        struct PathGuard {
            _lock: std::sync::MutexGuard<'static, ()>,
            saved: Option<std::ffi::OsString>,
        }

        impl PathGuard {
            fn set(value: &Path) -> Self {
                let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
                let saved = std::env::var_os("PATH");
                std::env::set_var("PATH", value);
                Self { _lock: lock, saved }
            }
        }

        impl Drop for PathGuard {
            fn drop(&mut self) {
                match &self.saved {
                    Some(v) => std::env::set_var("PATH", v),
                    None => std::env::remove_var("PATH"),
                }
            }
        }

        fn create_executable(path: &Path) {
            std::fs::write(path, "#!/bin/sh\nexit 0\n").unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        #[test]
        fn infer_resolves_bare_command_via_path() {
            let dir = tempfile::tempdir().unwrap();
            let bin = dir.path().join("widget");
            create_executable(&bin);
            let _g = PathGuard::set(dir.path());

            let resolved = resolve_executable(std::ffi::OsStr::new("widget"), None).unwrap();
            assert_eq!(resolved, bin);
        }

        #[test]
        fn infer_resolves_relative_path_against_cwd() {
            let dir = tempfile::tempdir().unwrap();
            let bin = dir.path().join("script");
            create_executable(&bin);

            let resolved =
                resolve_executable(std::ffi::OsStr::new("./script"), Some(dir.path())).unwrap();
            assert!(resolved.is_absolute());
            assert!(
                resolved.ends_with("script"),
                "expected resolved to end with 'script', got {resolved:?}"
            );
        }

        #[test]
        fn infer_passes_through_absolute_path() {
            let resolved = resolve_executable(std::ffi::OsStr::new("/bin/echo"), None).unwrap();
            assert_eq!(resolved, PathBuf::from("/bin/echo"));
        }

        #[test]
        fn infer_errors_on_missing_command() {
            let dir = tempfile::tempdir().unwrap();
            // PATH points at an empty dir — no `nonesuch` anywhere.
            let _g = PathGuard::set(dir.path());

            let err = resolve_executable(
                std::ffi::OsStr::new("definitely-not-a-real-binary-xyz"),
                None,
            )
            .unwrap_err();
            assert!(
                err.to_string().contains("not found in PATH"),
                "unexpected error: {err}"
            );
        }
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

    fn command_args(cmd: &lockin::SandboxedCommand) -> Vec<String> {
        cmd.as_command()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn normalized_command_args(cmd: &lockin::SandboxedCommand) -> Vec<String> {
        let private_tmp = cmd
            .as_command()
            .get_envs()
            .find_map(|(k, v)| {
                (k == "TMPDIR")
                    .then(|| v.map(|v| v.to_string_lossy().into_owned()))
                    .flatten()
            })
            .expect("sandbox command should set TMPDIR");
        command_args(cmd)
            .into_iter()
            .map(|arg| arg.replace(&private_tmp, "$LOCKIN_TMP"))
            .collect()
    }

    fn env_value(cmd: &lockin::SandboxedCommand, name: &str) -> Option<OsString> {
        cmd.as_command()
            .get_envs()
            .find_map(|(k, v)| (k == name).then(|| v.map(|v| v.to_owned())).flatten())
    }

    fn enforced_command(
        config: &Config,
        args: &[OsString],
        current_dir: Option<&Path>,
        network: lockin::NetworkMode,
        parent_env: Vec<(OsString, OsString)>,
        extra_env: Vec<(OsString, OsString)>,
    ) -> lockin::SandboxedCommand {
        build_enforced_command(EnforcedCommandSpec {
            builder: lockin::Sandbox::builder(),
            config,
            config_dir: None,
            program: Path::new("/bin/echo"),
            args,
            current_dir,
            network,
            parent_env,
            extra_env,
        })
        .unwrap()
    }

    #[test]
    fn apply_config_to_builder_is_composable() {
        let config = Config {
            filesystem: FilesystemConfig {
                read_paths: vec![PathBuf::from("/etc/hosts")],
                read_dirs: vec![PathBuf::from("/usr/share")],
                ..Default::default()
            },
            limits: LimitsConfig {
                max_open_files: Some(64),
                ..Default::default()
            },
            ..Default::default()
        };

        let wrapper_cmd = apply_config(&config, None)
            .unwrap()
            .command(Path::new("/bin/echo"))
            .unwrap();
        let composable_cmd = apply_config_to_builder(lockin::Sandbox::builder(), &config, None)
            .unwrap()
            .command(Path::new("/bin/echo"))
            .unwrap();

        assert_eq!(
            normalized_command_args(&wrapper_cmd),
            normalized_command_args(&composable_cmd)
        );
    }

    #[test]
    fn build_enforced_command_applies_args() {
        let config = Config::default();
        let args = vec![os("a"), os("b")];
        let cmd = enforced_command(
            &config,
            &args,
            None,
            lockin::NetworkMode::Deny,
            vec![],
            vec![],
        );
        let rendered = command_args(&cmd);
        assert!(rendered.ends_with(&["a".to_string(), "b".to_string()]));
    }

    #[test]
    fn build_enforced_command_applies_current_dir() {
        let config = Config::default();
        let dir = tempfile::tempdir().unwrap();
        let cmd = enforced_command(
            &config,
            &[],
            Some(dir.path()),
            lockin::NetworkMode::Deny,
            vec![],
            vec![],
        );
        assert_eq!(cmd.as_command().get_current_dir(), Some(dir.path()));
    }

    #[test]
    fn build_enforced_command_applies_env_policy() {
        let config = Config {
            env: EnvConfig {
                inherit: false,
                pass: vec!["FOO".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let cmd = enforced_command(
            &config,
            &[],
            None,
            lockin::NetworkMode::Deny,
            vec![(os("FOO"), os("1")), (os("BAR"), os("2"))],
            vec![],
        );
        assert_eq!(env_value(&cmd, "FOO"), Some(os("1")));
        assert_eq!(env_value(&cmd, "BAR"), None);
    }

    #[test]
    fn build_enforced_command_applies_extra_env_after_env_policy() {
        let config = Config::default();
        let cmd = enforced_command(
            &config,
            &[],
            None,
            lockin::NetworkMode::Deny,
            vec![],
            vec![(os("HTTP_PROXY"), os("http://x"))],
        );
        assert_eq!(env_value(&cmd, "HTTP_PROXY"), Some(os("http://x")));
    }

    #[test]
    fn build_enforced_command_applies_network() {
        let config = Config::default();
        let deny = enforced_command(
            &config,
            &[],
            None,
            lockin::NetworkMode::Deny,
            vec![],
            vec![],
        );
        let allow = enforced_command(
            &config,
            &[],
            None,
            lockin::NetworkMode::AllowAll,
            vec![],
            vec![],
        );
        assert_ne!(
            normalized_command_args(&deny),
            normalized_command_args(&allow)
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
