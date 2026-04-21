mod config;
mod glob;

use std::ffi::OsString;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{ExitCode, ExitStatus};

use anyhow::Context;
use clap::Parser;

use config::{apply_config, load_config, resolve_command, resolve_network_plan, NetworkPlan};

const EXIT_LOCKIN_ERROR: u8 = 125;

const BUILTIN_ENV_BLOCKLIST: &[&str] = &[
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

const SANDBOX_OWNED_ENV: &[&str] = &["TMPDIR", "TMP", "TEMP"];

#[derive(Parser, Debug)]
#[command(name = "lockin", about = "Run programs inside an OS sandbox")]
#[command(trailing_var_arg = true)]
struct Cli {
    #[arg(short = 'c', long = "config")]
    config: Option<PathBuf>,

    command: Vec<OsString>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("lockin: {e:#}");
            ExitCode::from(EXIT_LOCKIN_ERROR)
        }
    }
}

fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    let config = resolve_config(&cli.config)?;
    let (program, args) = resolve_command(&config, &cli.command)?;

    let proxy = ProxyLifecycle::start(resolve_network_plan(&config)?)?;
    let network_mode = proxy.sandbox_mode();

    let mut cmd = apply_config(&config)?
        .network(network_mode)
        .command(&program)?;
    cmd.args(&args);
    apply_env(&config.env, &mut cmd, std::env::vars_os());
    proxy.inject_env(&mut cmd);

    let status = cmd.status()?;
    drop(proxy);
    Ok(ExitCode::from(child_exit_code(status)))
}

/// Owns the tokio runtime and `outpost-proxy` handle when the child
/// is running in proxy mode. Dropping this value shuts the proxy
/// down, so it must outlive `cmd.status()`.
struct ProxyLifecycle {
    proxy: Option<ActiveProxy>,
    mode: lockin::NetworkMode,
}

struct ActiveProxy {
    // Field order matters for Drop: the handle must drop before the
    // runtime it was spawned on. Rust drops struct fields in
    // declaration order, top to bottom.
    _handle: outpost_proxy::ProxyHandle,
    _runtime: tokio::runtime::Runtime,
    port: u16,
}

impl ProxyLifecycle {
    fn start(plan: NetworkPlan) -> anyhow::Result<Self> {
        match plan {
            NetworkPlan::Deny => Ok(Self {
                proxy: None,
                mode: lockin::NetworkMode::Deny,
            }),
            NetworkPlan::AllowAll => Ok(Self {
                proxy: None,
                mode: lockin::NetworkMode::AllowAll,
            }),
            NetworkPlan::Proxy { policy } => {
                // Multi-thread runtime so the proxy keeps driving
                // while main thread blocks on `cmd.status()`.
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .context("failed to build tokio runtime for outpost-proxy")?;
                let handle = runtime
                    .block_on(outpost_proxy::start(policy))
                    .context("failed to start outpost-proxy daemon")?;
                let port = handle.listen_addr().port();
                Ok(Self {
                    proxy: Some(ActiveProxy {
                        _handle: handle,
                        _runtime: runtime,
                        port,
                    }),
                    mode: lockin::NetworkMode::Proxy {
                        loopback_port: port,
                    },
                })
            }
        }
    }

    fn sandbox_mode(&self) -> lockin::NetworkMode {
        self.mode
    }

    /// Writes `HTTP_PROXY`/`HTTPS_PROXY`/`ALL_PROXY` and clears
    /// `NO_PROXY` on the child command so every standard HTTP client
    /// (libcurl, Python requests, Go net/http) routes through the
    /// loopback proxy. No-op when not in proxy mode.
    fn inject_env(&self, cmd: &mut lockin::SandboxCommand) {
        if let Some(active) = &self.proxy {
            let url = format!("http://127.0.0.1:{}", active.port);
            cmd.env("HTTP_PROXY", &url)
                .env("HTTPS_PROXY", &url)
                .env("http_proxy", &url)
                .env("https_proxy", &url)
                .env("ALL_PROXY", &url)
                .env("all_proxy", &url)
                .env("NO_PROXY", "")
                .env("no_proxy", "");
        }
    }
}

// Non-UTF-8 env keys are skipped in pass matching and block filtering;
// glob matching is byte-level ASCII.
fn apply_env<I>(env: &config::EnvConfig, cmd: &mut lockin::SandboxCommand, parent_env: I)
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    let parent: Vec<(OsString, OsString)> = parent_env.into_iter().collect();
    let blocklist: Vec<&str> = BUILTIN_ENV_BLOCKLIST
        .iter()
        .copied()
        .chain(env.block.iter().map(String::as_str))
        .collect();
    let is_blocked = |name: &str| blocklist.iter().any(|p| glob::matches(p, name));

    if env.inherit {
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
            if env.pass.iter().any(|p| glob::matches(p, name)) && !is_blocked(name) {
                cmd.env(key, value);
            }
        }
    }

    for (key, value) in &env.set {
        if !is_blocked(key) {
            cmd.env(key, value);
        }
    }
}

fn child_exit_code(status: ExitStatus) -> u8 {
    if let Some(code) = status.code() {
        return code as u8;
    }
    if let Some(sig) = status.signal() {
        return (128 + sig) as u8;
    }
    EXIT_LOCKIN_ERROR
}

fn resolve_config(explicit: &Option<PathBuf>) -> anyhow::Result<config::Config> {
    if let Some(path) = explicit {
        return load_config(path);
    }

    let default_path = Path::new("lockin.toml");
    if default_path.exists() {
        return load_config(default_path);
    }

    Ok(config::Config::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(args)
    }

    #[test]
    fn config_and_command_with_separator() {
        let cli = parse(&["lockin", "-c", "sandbox.toml", "--", "myapp", "--flag"]).unwrap();
        assert_eq!(cli.config.unwrap(), PathBuf::from("sandbox.toml"));
        assert_eq!(cli.command, vec!["myapp", "--flag"]);
    }

    #[test]
    fn short_config_attached_value() {
        let cli = parse(&["lockin", "-csandbox.toml", "script.py", "--verbose"]).unwrap();
        assert_eq!(cli.config.unwrap(), PathBuf::from("sandbox.toml"));
        assert_eq!(cli.command, vec!["script.py", "--verbose"]);
    }

    #[test]
    fn no_config_with_command() {
        let cli = parse(&["lockin", "--", "myapp", "--flag"]).unwrap();
        assert!(cli.config.is_none());
        assert_eq!(cli.command, vec!["myapp", "--flag"]);
    }

    #[test]
    fn no_config_no_separator() {
        let cli = parse(&["lockin", "myapp"]).unwrap();
        assert!(cli.config.is_none());
        assert_eq!(cli.command, vec!["myapp"]);
    }

    #[test]
    fn long_config_flag() {
        let cli = parse(&["lockin", "--config", "sandbox.toml", "--", "myapp"]).unwrap();
        assert_eq!(cli.config.unwrap(), PathBuf::from("sandbox.toml"));
        assert_eq!(cli.command, vec!["myapp"]);
    }

    #[test]
    fn shebang_argv_simulation() {
        let cli = parse(&[
            "/usr/bin/lockin",
            "-c/etc/lockin/python3.toml",
            "/home/user/script.py",
            "--user",
            "alice",
        ])
        .unwrap();
        assert_eq!(
            cli.config.unwrap(),
            PathBuf::from("/etc/lockin/python3.toml")
        );
        assert_eq!(cli.command, vec!["/home/user/script.py", "--user", "alice"]);
    }

    #[test]
    fn hyphen_values_after_separator() {
        let cli = parse(&["lockin", "-c", "sandbox.toml", "--", "--extra-flag"]).unwrap();
        assert_eq!(cli.config.unwrap(), PathBuf::from("sandbox.toml"));
        assert_eq!(cli.command, vec!["--extra-flag"]);
    }

    #[test]
    fn trailing_flags_after_positional() {
        let cli = parse(&[
            "lockin",
            "-csandbox.toml",
            "script.py",
            "--verbose",
            "--user",
            "alice",
        ])
        .unwrap();
        assert_eq!(cli.config.unwrap(), PathBuf::from("sandbox.toml"));
        assert_eq!(
            cli.command,
            vec!["script.py", "--verbose", "--user", "alice"]
        );
    }

    fn build_cmd() -> lockin::SandboxCommand {
        lockin::Sandbox::builder()
            .command(Path::new("/bin/echo"))
            .unwrap()
    }

    fn removed_keys(cmd: &lockin::SandboxCommand) -> Vec<OsString> {
        cmd.as_command()
            .get_envs()
            .filter(|(_, v)| v.is_none())
            .map(|(k, _)| k.to_owned())
            .collect()
    }

    fn synthetic_env(keys: &[&str]) -> Vec<(OsString, OsString)> {
        keys.iter()
            .map(|k| (OsString::from(k), OsString::from("value")))
            .collect()
    }

    fn set_pairs(cmd: &lockin::SandboxCommand) -> Vec<(OsString, OsString)> {
        cmd.as_command()
            .get_envs()
            .filter_map(|(k, v)| v.map(|v| (k.to_owned(), v.to_owned())))
            .collect()
    }

    #[test]
    fn apply_env_strips_builtin_blocklist() {
        let mut cmd = build_cmd();
        let mut parent: Vec<&str> = BUILTIN_ENV_BLOCKLIST.to_vec();
        parent.push("UNRELATED");
        let env_config = config::EnvConfig {
            inherit: true,
            ..Default::default()
        };
        apply_env(&env_config, &mut cmd, synthetic_env(&parent));
        let removed = removed_keys(&cmd);
        for var in BUILTIN_ENV_BLOCKLIST {
            assert!(
                removed.iter().any(|k| k == var),
                "expected {var} removed, got: {removed:?}"
            );
        }
        assert!(!removed.iter().any(|k| k == "UNRELATED"));
    }

    #[test]
    fn apply_env_respects_user_block_patterns() {
        let mut cmd = build_cmd();
        let env_config = config::EnvConfig {
            inherit: true,
            block: vec!["AWS_*".into(), "GITHUB_TOKEN".into()],
            ..Default::default()
        };
        apply_env(
            &env_config,
            &mut cmd,
            synthetic_env(&["AWS_SECRET", "AWS_SESSION_TOKEN", "GITHUB_TOKEN", "OTHER"]),
        );
        let removed = removed_keys(&cmd);
        assert!(removed.iter().any(|k| k == "AWS_SECRET"));
        assert!(removed.iter().any(|k| k == "AWS_SESSION_TOKEN"));
        assert!(removed.iter().any(|k| k == "GITHUB_TOKEN"));
        assert!(!removed.iter().any(|k| k == "OTHER"));
    }

    #[test]
    fn apply_env_pass_ignored_when_inherit_true() {
        let mut cmd = build_cmd();
        let before = set_pairs(&cmd);
        let env_config = config::EnvConfig {
            inherit: true,
            pass: vec!["HOME".into()],
            ..Default::default()
        };
        apply_env(&env_config, &mut cmd, synthetic_env(&["HOME", "OTHER"]));
        let after = set_pairs(&cmd);
        assert_eq!(before, after, "pass must be a no-op when inherit=true");
    }

    #[test]
    fn apply_env_pass_imports_matched_parent_vars() {
        let mut cmd = build_cmd();
        let env_config = config::EnvConfig {
            inherit: false,
            pass: vec!["PATH".into(), "HOME".into()],
            ..Default::default()
        };
        let parent: Vec<(OsString, OsString)> = vec![
            ("PATH".into(), "/bin:/usr/bin".into()),
            ("HOME".into(), "/home/u".into()),
            ("UNRELATED".into(), "x".into()),
        ];
        apply_env(&env_config, &mut cmd, parent);
        let set = set_pairs(&cmd);
        assert!(set.iter().any(|(k, v)| k == "PATH" && v == "/bin:/usr/bin"));
        assert!(set.iter().any(|(k, v)| k == "HOME" && v == "/home/u"));
        assert!(!set.iter().any(|(k, _)| k == "UNRELATED"));
    }

    #[test]
    fn apply_env_pass_supports_globs() {
        let mut cmd = build_cmd();
        let env_config = config::EnvConfig {
            inherit: false,
            pass: vec!["NIX_*".into()],
            ..Default::default()
        };
        apply_env(
            &env_config,
            &mut cmd,
            synthetic_env(&["NIX_CC", "NIX_CFLAGS", "OTHER"]),
        );
        let set = set_pairs(&cmd);
        assert!(set.iter().any(|(k, _)| k == "NIX_CC"));
        assert!(set.iter().any(|(k, _)| k == "NIX_CFLAGS"));
        assert!(!set.iter().any(|(k, _)| k == "OTHER"));
    }

    #[test]
    fn apply_env_set_adds_hardcoded_values() {
        let mut cmd = build_cmd();
        let env_config = config::EnvConfig {
            inherit: false,
            set: [("LANG".into(), "C.UTF-8".into())].into(),
            ..Default::default()
        };
        apply_env(&env_config, &mut cmd, synthetic_env(&[]));
        let set = set_pairs(&cmd);
        assert!(set.iter().any(|(k, v)| k == "LANG" && v == "C.UTF-8"));
    }

    #[test]
    fn apply_env_set_overrides_pass_on_collision() {
        let mut cmd = build_cmd();
        let env_config = config::EnvConfig {
            inherit: false,
            pass: vec!["TERM".into()],
            set: [("TERM".into(), "dumb".into())].into(),
            ..Default::default()
        };
        let parent: Vec<(OsString, OsString)> = vec![("TERM".into(), "xterm-256color".into())];
        apply_env(&env_config, &mut cmd, parent);
        let set = set_pairs(&cmd);
        let term = set.iter().find(|(k, _)| k == "TERM");
        assert_eq!(term.map(|(_, v)| v.to_str()), Some(Some("dumb")));
    }

    #[test]
    fn apply_env_block_strips_set_entries() {
        let mut cmd = build_cmd();
        let env_config = config::EnvConfig {
            inherit: true,
            set: [("AWS_KEY".into(), "leak".into())].into(),
            block: vec!["AWS_*".into()],
            ..Default::default()
        };
        apply_env(&env_config, &mut cmd, synthetic_env(&[]));
        let set = set_pairs(&cmd);
        assert!(!set.iter().any(|(k, _)| k == "AWS_KEY"));
    }

    #[test]
    fn apply_env_builtin_blocklist_strips_set_entries() {
        let mut cmd = build_cmd();
        let first_builtin = BUILTIN_ENV_BLOCKLIST[0];
        let env_config = config::EnvConfig {
            inherit: false,
            set: [(first_builtin.into(), "/evil".into())].into(),
            ..Default::default()
        };
        apply_env(&env_config, &mut cmd, synthetic_env(&[]));
        let set = set_pairs(&cmd);
        assert!(
            !set.iter().any(|(k, _)| k == first_builtin),
            "built-in blocklist must override set"
        );
    }

    #[test]
    fn apply_env_block_strips_pass_imported() {
        let mut cmd = build_cmd();
        let env_config = config::EnvConfig {
            inherit: false,
            pass: vec!["SECRET".into()],
            block: vec!["SECRET".into()],
            ..Default::default()
        };
        apply_env(&env_config, &mut cmd, synthetic_env(&["SECRET"]));
        let set = set_pairs(&cmd);
        assert!(!set.iter().any(|(k, _)| k == "SECRET"));
    }

    #[test]
    fn apply_env_inherit_false_preserves_sandbox_env_across_clear() {
        let mut cmd = build_cmd();
        let sandbox_env = set_pairs(&cmd);
        assert!(
            !sandbox_env.is_empty(),
            "sandbox library should have set some env (TMPDIR etc.)"
        );
        apply_env(
            &config::EnvConfig {
                inherit: false,
                ..Default::default()
            },
            &mut cmd,
            synthetic_env(&["FOO", "BAR"]),
        );
        assert_eq!(
            set_pairs(&cmd),
            sandbox_env,
            "inherit=false keeps only TMPDIR/TMP/TEMP from the sandbox library"
        );
    }

    #[test]
    fn child_exit_code_normal() {
        let status = ExitStatus::from_raw(0 << 8);
        assert_eq!(child_exit_code(status), 0);
        let status = ExitStatus::from_raw(42 << 8);
        assert_eq!(child_exit_code(status), 42);
    }

    #[test]
    fn child_exit_code_signal() {
        let status = ExitStatus::from_raw(9);
        assert_eq!(child_exit_code(status), 128 + 9);
        let status = ExitStatus::from_raw(15);
        assert_eq!(child_exit_code(status), 128 + 15);
    }

    #[test]
    fn empty_command_is_valid_parse() {
        let cli = parse(&["lockin"]).unwrap();
        assert!(cli.config.is_none());
        assert!(cli.command.is_empty());
    }
}
