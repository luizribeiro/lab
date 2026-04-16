mod config;

use std::ffi::OsString;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{ExitCode, ExitStatus};

use clap::Parser;

use config::{apply_config, load_config, resolve_command};

const EXIT_LOCKIN_ERROR: u8 = 125;

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
    let status = cmd.status()?;
    Ok(ExitCode::from(child_exit_code(status)))
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
