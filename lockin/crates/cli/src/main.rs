#[allow(dead_code)]
mod config;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "lockin", about = "Run programs inside an OS sandbox")]
#[command(trailing_var_arg = true)]
struct Cli {
    #[arg(short = 'c', long = "config")]
    config: Option<PathBuf>,

    command: Vec<OsString>,
}

fn main() -> ExitCode {
    let _cli = Cli::parse();
    ExitCode::SUCCESS
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
    fn empty_command_is_valid_parse() {
        let cli = parse(&["lockin"]).unwrap();
        assert!(cli.config.is_none());
        assert!(cli.command.is_empty());
    }
}
