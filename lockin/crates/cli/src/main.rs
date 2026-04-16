mod config;
mod glob;

use std::ffi::OsString;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{ExitCode, ExitStatus};

use clap::Parser;

use config::{apply_config, load_config, resolve_command};

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
    let mut cmd = apply_config(&config)?.command(&program)?;
    cmd.args(&args);
    apply_env(&config.env, &mut cmd, std::env::vars_os());
    let status = cmd.status()?;
    Ok(ExitCode::from(child_exit_code(status)))
}

fn apply_env<I>(env: &config::EnvConfig, cmd: &mut lockin::SandboxCommand, parent_env: I)
where
    I: IntoIterator<Item = (OsString, OsString)>,
{
    if !env.inherit {
        cmd.env_clear();
        return;
    }
    for (key, _) in parent_env {
        let Some(name) = key.to_str() else { continue };
        let matches_any = BUILTIN_ENV_BLOCKLIST
            .iter()
            .copied()
            .chain(env.block.iter().map(String::as_str))
            .any(|p| glob::matches(p, name));
        if matches_any {
            cmd.env_remove(&key);
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

    #[test]
    fn apply_env_strips_builtin_blocklist() {
        let mut cmd = build_cmd();
        let mut parent: Vec<&str> = BUILTIN_ENV_BLOCKLIST.to_vec();
        parent.push("UNRELATED");
        apply_env(
            &config::EnvConfig::default(),
            &mut cmd,
            synthetic_env(&parent),
        );
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
    fn apply_env_inherit_false_clears_prior_overrides() {
        let mut cmd = build_cmd();
        cmd.env("PRESET", "value");
        assert!(cmd.as_command().get_envs().any(|(k, _)| k == "PRESET"));
        apply_env(
            &config::EnvConfig {
                inherit: false,
                block: vec![],
            },
            &mut cmd,
            synthetic_env(&["LD_PRELOAD"]),
        );
        assert_eq!(
            cmd.as_command().get_envs().count(),
            0,
            "env_clear should have dropped all overrides"
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
