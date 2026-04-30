use lockin_config as config;

use std::ffi::OsString;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{ExitCode, ExitStatus};

use anyhow::Context;
use clap::Parser;

use config::{
    apply_config, apply_env, load_config, resolve_command, resolve_executable,
    resolve_network_plan, NetworkPlan,
};

const EXIT_LOCKIN_ERROR: u8 = 125;

#[derive(Parser, Debug)]
#[command(name = "lockin", about = "Run programs inside an OS sandbox")]
#[command(trailing_var_arg = true)]
struct Cli {
    #[arg(short = 'c', long = "config")]
    config: Option<PathBuf>,

    command: Vec<OsString>,
}

#[derive(Parser, Debug)]
#[command(
    name = "lockin infer",
    about = "Run a program under observation and emit a starter lockin.toml",
    trailing_var_arg = true
)]
struct InferCli {
    /// Path to write the inferred lockin.toml.
    #[arg(short = 'o', long = "output")]
    output: PathBuf,

    /// Optional seed config; observed entries are merged into it.
    #[arg(short = 'c', long = "config")]
    config: Option<PathBuf>,

    /// The program to observe and its arguments.
    #[arg(required = true)]
    command: Vec<OsString>,
}

#[derive(Parser, Debug)]
#[command(
    name = "lockin trace",
    about = "Run a program under your lockin.toml policy and record what got denied",
    trailing_var_arg = true
)]
struct TraceCli {
    /// Path to write the denial log. Default `./lockin-denials.log`.
    #[arg(short = 'o', long = "output", default_value = "lockin-denials.log")]
    output: PathBuf,

    /// lockin.toml to enforce. If omitted, uses the same resolution as
    /// run mode (./lockin.toml if present, else deny-all default).
    #[arg(short = 'c', long = "config")]
    config: Option<PathBuf>,

    /// The program to trace and its arguments.
    #[arg(required = true)]
    command: Vec<OsString>,
}

/// Returns true if the given argv is intended for the `infer`
/// subcommand. Pulled out for test coverage.
fn is_infer_invocation(argv: &[OsString]) -> bool {
    argv.get(1).map(|s| s == "infer").unwrap_or(false)
}

/// Returns true if the given argv is intended for the `trace`
/// subcommand. Pulled out for test coverage.
fn is_trace_invocation(argv: &[OsString]) -> bool {
    argv.get(1).map(|s| s == "trace").unwrap_or(false)
}

fn main() -> ExitCode {
    let argv: Vec<OsString> = std::env::args_os().collect();
    if is_infer_invocation(&argv) {
        let infer_argv: Vec<OsString> = std::iter::once(argv[0].clone())
            .chain(argv.into_iter().skip(2))
            .collect();
        match InferCli::try_parse_from(&infer_argv) {
            Ok(cli) => return run_infer(cli),
            Err(e) => {
                let _ = e.print();
                return ExitCode::from(EXIT_LOCKIN_ERROR);
            }
        }
    }
    if is_trace_invocation(&argv) {
        let trace_argv: Vec<OsString> = std::iter::once(argv[0].clone())
            .chain(argv.into_iter().skip(2))
            .collect();
        match TraceCli::try_parse_from(&trace_argv) {
            Ok(cli) => return run_trace(cli),
            Err(e) => {
                let _ = e.print();
                return ExitCode::from(EXIT_LOCKIN_ERROR);
            }
        }
    }
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("lockin: {e:#}");
            ExitCode::from(EXIT_LOCKIN_ERROR)
        }
    }
}

fn run_infer(cli: InferCli) -> ExitCode {
    match do_infer(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("lockin infer: {e:#}");
            ExitCode::from(EXIT_LOCKIN_ERROR)
        }
    }
}

fn do_infer(cli: InferCli) -> anyhow::Result<ExitCode> {
    let mut command = cli.command;
    if command.first().map(|s| s == "--").unwrap_or(false) {
        command.remove(0);
    }
    if command.is_empty() {
        anyhow::bail!("missing program; usage: lockin infer -o OUT [-c SEED] -- program args...");
    }

    let seed = match &cli.config {
        Some(path) => {
            let (cfg, _dir) = load_config(path)?;
            Some(cfg)
        }
        None => None,
    };

    let program = resolve_executable(command[0].as_os_str(), None)
        .with_context(|| format!("resolving program {:?}", command[0]))?;
    let args: Vec<OsString> = command[1..].to_vec();

    let request = lockin_infer::InferRequest {
        program,
        args,
        current_dir: None,
        env: vec![],
    };
    let options = lockin_infer::InferOptions {
        seed,
        output: Some(cli.output),
    };

    let report = lockin_infer::infer(request, options)?;

    for d in &report.diagnostics {
        match d.level {
            lockin_infer::DiagnosticLevel::Error => {
                eprintln!("lockin infer: error: {}", d.message)
            }
            lockin_infer::DiagnosticLevel::Warn => {
                eprintln!("lockin infer: warning: {}", d.message)
            }
            lockin_infer::DiagnosticLevel::Info => {
                eprintln!("lockin infer: {}", d.message)
            }
        }
    }

    Ok(ExitCode::from(child_exit_code(report.status)))
}

fn run_trace(cli: TraceCli) -> ExitCode {
    match do_trace(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("lockin trace: {e:#}");
            ExitCode::from(EXIT_LOCKIN_ERROR)
        }
    }
}

fn do_trace(cli: TraceCli) -> anyhow::Result<ExitCode> {
    let mut command = cli.command;
    if command.first().map(|s| s == "--").unwrap_or(false) {
        command.remove(0);
    }
    if command.is_empty() {
        anyhow::bail!(
            "missing program; usage: lockin trace [-o OUT] [-c CONFIG] -- program args..."
        );
    }

    let (config, config_dir) = resolve_config(&cli.config)?;

    let program = resolve_executable(command[0].as_os_str(), None)
        .with_context(|| format!("resolving program {:?}", command[0]))?;
    let args: Vec<OsString> = command[1..].to_vec();

    eprintln!(
        "lockin trace: recording denials to {}",
        cli.output.display()
    );

    // Trace mode mirrors run mode's network/env story: a Proxy plan
    // needs the outpost-proxy daemon spinning before the sandbox starts,
    // and the resolved NetworkMode + proxy env are threaded onto the
    // TraceRequest so the runner's builder reflects the user's policy
    // verbatim.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .context("failed to build tokio runtime")?;
    let proxy = ProxyLifecycle::start(runtime.handle(), resolve_network_plan(&config)?)?;

    let request = lockin_trace::TraceRequest {
        program,
        args,
        current_dir: None,
        env: proxy.env_pairs(),
        config,
        config_dir,
        network: proxy.sandbox_mode(),
    };
    let options = lockin_trace::TraceOptions {
        output: Some(cli.output.clone()),
    };

    let report = lockin_trace::trace(request, options)?;

    for d in &report.diagnostics {
        eprintln!("lockin trace: {}", d.message);
    }
    eprintln!(
        "lockin trace: {} denial(s) recorded in {}",
        report.denials.len(),
        cli.output.display()
    );

    drop(proxy);
    drop(runtime);
    Ok(ExitCode::from(child_exit_code(report.status)))
}

fn run(cli: Cli) -> anyhow::Result<ExitCode> {
    let (config, config_dir) = resolve_config(&cli.config)?;
    let (program, args) = resolve_command(&config, &cli.command, config_dir.as_deref())?;

    // One runtime drives both outpost-proxy (when configured) and the
    // signal-forwarding supervisor (always).
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
        .context("failed to build tokio runtime")?;

    let proxy = ProxyLifecycle::start(runtime.handle(), resolve_network_plan(&config)?)?;
    let network_mode = proxy.sandbox_mode();

    let mut cmd = apply_config(&config, config_dir.as_deref())?
        .network(network_mode)
        .command(&program)?;
    cmd.args(&args);
    apply_env(&config.env, &mut cmd, std::env::vars_os());
    proxy.inject_env(&mut cmd);

    let status = lockin::supervise::supervise_command(cmd, runtime.handle())?;

    drop(proxy);
    drop(runtime);
    Ok(ExitCode::from(child_exit_code(status)))
}

/// Holds the `outpost-proxy` handle for proxy-mode runs, plus the
/// resolved sandbox network mode. Dropping this value shuts the
/// proxy down, so it must outlive the supervised child.
struct ProxyLifecycle {
    handle: Option<outpost_proxy::ProxyHandle>,
    mode: lockin::NetworkMode,
}

impl ProxyLifecycle {
    fn start(rt: &tokio::runtime::Handle, plan: NetworkPlan) -> anyhow::Result<Self> {
        match plan {
            NetworkPlan::Deny => Ok(Self {
                handle: None,
                mode: lockin::NetworkMode::Deny,
            }),
            NetworkPlan::AllowAll => Ok(Self {
                handle: None,
                mode: lockin::NetworkMode::AllowAll,
            }),
            NetworkPlan::Proxy { policy } => {
                let handle = rt
                    .block_on(outpost_proxy::start(policy))
                    .context("failed to start outpost-proxy daemon")?;
                let port = handle.listen_addr().port();
                Ok(Self {
                    handle: Some(handle),
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
    fn inject_env(&self, cmd: &mut lockin::SandboxedCommand) {
        for (k, v) in self.env_pairs() {
            cmd.env(k, v);
        }
    }

    /// Same env contract as `inject_env`, but as an owned key/value
    /// vector so callers that don't yet have a `SandboxedCommand` (the
    /// trace runner builds it inside its own crate) can pass it in.
    fn env_pairs(&self) -> Vec<(OsString, OsString)> {
        let Some(handle) = &self.handle else {
            return Vec::new();
        };
        let url = format!("http://127.0.0.1:{}", handle.listen_addr().port());
        [
            ("HTTP_PROXY", url.as_str()),
            ("HTTPS_PROXY", url.as_str()),
            ("http_proxy", url.as_str()),
            ("https_proxy", url.as_str()),
            ("ALL_PROXY", url.as_str()),
            ("all_proxy", url.as_str()),
            ("NO_PROXY", ""),
            ("no_proxy", ""),
        ]
        .into_iter()
        .map(|(k, v)| (OsString::from(k), OsString::from(v)))
        .collect()
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

fn resolve_config(explicit: &Option<PathBuf>) -> anyhow::Result<(config::Config, Option<PathBuf>)> {
    if let Some(path) = explicit {
        let (config, dir) = load_config(path)?;
        return Ok((config, Some(dir)));
    }

    let default_path = Path::new("lockin.toml");
    if default_path.exists() {
        let (config, dir) = load_config(default_path)?;
        return Ok((config, Some(dir)));
    }

    Ok((config::Config::default(), None))
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

    fn build_cmd() -> lockin::SandboxedCommand {
        lockin::Sandbox::builder()
            .command(Path::new("/bin/echo"))
            .unwrap()
    }

    fn removed_keys(cmd: &lockin::SandboxedCommand) -> Vec<OsString> {
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

    fn set_pairs(cmd: &lockin::SandboxedCommand) -> Vec<(OsString, OsString)> {
        cmd.as_command()
            .get_envs()
            .filter_map(|(k, v)| v.map(|v| (k.to_owned(), v.to_owned())))
            .collect()
    }

    #[test]
    fn apply_env_strips_builtin_blocklist() {
        let mut cmd = build_cmd();
        let mut parent: Vec<&str> = config::BUILTIN_ENV_BLOCKLIST.to_vec();
        parent.push("UNRELATED");
        let env_config = config::EnvConfig {
            inherit: true,
            ..Default::default()
        };
        apply_env(&env_config, &mut cmd, synthetic_env(&parent));
        let removed = removed_keys(&cmd);
        for var in config::BUILTIN_ENV_BLOCKLIST {
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
        let first_builtin = config::BUILTIN_ENV_BLOCKLIST[0];
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

    fn parse_infer(args: &[&str]) -> Result<InferCli, clap::Error> {
        InferCli::try_parse_from(args)
    }

    fn argv(args: &[&str]) -> Vec<OsString> {
        args.iter().map(OsString::from).collect()
    }

    #[test]
    fn infer_dispatches_when_first_arg_is_infer() {
        assert!(is_infer_invocation(&argv(&[
            "lockin", "infer", "-o", "out.toml"
        ])));
        assert!(is_infer_invocation(&argv(&[
            "lockin",
            "infer",
            "--",
            "/bin/echo"
        ])));
    }

    #[test]
    fn infer_dispatch_skips_when_dash_dash_first() {
        // `lockin -- infer` runs a program literally named `infer`.
        assert!(!is_infer_invocation(&argv(&[
            "lockin", "--", "infer", "args"
        ])));
    }

    #[test]
    fn infer_dispatch_skips_for_run_mode_invocations() {
        assert!(!is_infer_invocation(&argv(&["lockin", "/bin/echo", "hi"])));
        assert!(!is_infer_invocation(&argv(&[
            "lockin",
            "-c",
            "cfg.toml",
            "--",
            "/bin/echo",
            "hi"
        ])));
        assert!(!is_infer_invocation(&argv(&["lockin"])));
    }

    #[test]
    fn infer_parse_required_output_and_command() {
        let cli =
            parse_infer(&["lockin infer", "-o", "out.toml", "--", "/bin/echo", "hi"]).unwrap();
        assert_eq!(cli.output, PathBuf::from("out.toml"));
        assert_eq!(cli.config, None);
        // clap with trailing_var_arg keeps "--" out, but the leading "--"
        // is sometimes preserved depending on parser version. do_infer
        // strips a leading "--" defensively, so accept either form.
        let cmd: Vec<&OsString> = cli.command.iter().collect();
        assert!(
            cmd == vec![&OsString::from("/bin/echo"), &OsString::from("hi")]
                || cmd
                    == vec![
                        &OsString::from("--"),
                        &OsString::from("/bin/echo"),
                        &OsString::from("hi"),
                    ],
            "unexpected command parse: {:?}",
            cli.command
        );
    }

    #[test]
    fn infer_parse_seed_and_command() {
        let cli = parse_infer(&[
            "lockin infer",
            "-o",
            "out.toml",
            "-c",
            "seed.toml",
            "--",
            "./prog",
            "arg1",
            "arg2",
        ])
        .unwrap();
        assert_eq!(cli.output, PathBuf::from("out.toml"));
        assert_eq!(cli.config, Some(PathBuf::from("seed.toml")));
        assert!(cli.command.iter().any(|s| s == "./prog"));
        assert!(cli.command.iter().any(|s| s == "arg1"));
        assert!(cli.command.iter().any(|s| s == "arg2"));
    }

    #[test]
    fn infer_parse_no_args_errors() {
        assert!(parse_infer(&["lockin infer"]).is_err());
    }

    #[test]
    fn infer_parse_missing_output_errors() {
        assert!(parse_infer(&["lockin infer", "--", "/bin/echo"]).is_err());
    }

    #[test]
    fn run_mode_with_double_dash_keeps_program_named_infer() {
        // Validates the dispatch *and* parse: argv `lockin -- infer args`
        // is run mode, and command becomes ["infer", "args"].
        let argv: Vec<OsString> = argv(&["lockin", "--", "infer", "args"]);
        assert!(!is_infer_invocation(&argv));
        let cli = parse(&["lockin", "--", "infer", "args"]).unwrap();
        assert_eq!(cli.command, vec!["infer", "args"]);
    }

    fn parse_trace(args: &[&str]) -> Result<TraceCli, clap::Error> {
        TraceCli::try_parse_from(args)
    }

    #[test]
    fn trace_dispatches_when_first_arg_is_trace() {
        assert!(is_trace_invocation(&argv(&[
            "lockin",
            "trace",
            "-o",
            "denials.log"
        ])));
        assert!(is_trace_invocation(&argv(&[
            "lockin",
            "trace",
            "--",
            "/bin/echo"
        ])));
    }

    #[test]
    fn trace_dispatch_skips_when_dash_dash_first() {
        // `lockin -- trace` runs a program literally named `trace`.
        assert!(!is_trace_invocation(&argv(&[
            "lockin", "--", "trace", "args"
        ])));
    }

    #[test]
    fn trace_dispatch_skips_for_run_mode_invocations() {
        assert!(!is_trace_invocation(&argv(&["lockin", "/bin/echo", "hi"])));
        assert!(!is_trace_invocation(&argv(&["lockin"])));
        // `lockin infer ...` must dispatch as infer, not trace.
        assert!(!is_trace_invocation(&argv(&[
            "lockin", "infer", "-o", "x.toml"
        ])));
    }

    #[test]
    fn lockin_trace_with_program_and_args_parses() {
        let cli = parse_trace(&["lockin trace", "-o", "out.log", "--", "/bin/echo", "hi"]).unwrap();
        assert_eq!(cli.output, PathBuf::from("out.log"));
        assert_eq!(cli.config, None);
        let cmd: Vec<&OsString> = cli.command.iter().collect();
        assert!(
            cmd == vec![&OsString::from("/bin/echo"), &OsString::from("hi")]
                || cmd
                    == vec![
                        &OsString::from("--"),
                        &OsString::from("/bin/echo"),
                        &OsString::from("hi"),
                    ],
            "unexpected command parse: {:?}",
            cli.command
        );
    }

    #[test]
    fn lockin_trace_with_config_and_default_output() {
        let cli = parse_trace(&["lockin trace", "-c", "lockin.toml", "--", "./prog"]).unwrap();
        assert_eq!(cli.output, PathBuf::from("lockin-denials.log"));
        assert_eq!(cli.config, Some(PathBuf::from("lockin.toml")));
        assert!(cli.command.iter().any(|s| s == "./prog"));
    }

    #[test]
    fn lockin_trace_no_command_errors() {
        assert!(parse_trace(&["lockin trace"]).is_err());
    }

    #[test]
    fn lockin_trace_no_output_uses_default() {
        let cli = parse_trace(&["lockin trace", "--", "/bin/echo"]).unwrap();
        assert_eq!(cli.output, PathBuf::from("lockin-denials.log"));
    }

    #[test]
    fn run_mode_with_double_dash_keeps_program_named_trace() {
        let argv: Vec<OsString> = argv(&["lockin", "--", "trace", "args"]);
        assert!(!is_trace_invocation(&argv));
        let cli = parse(&["lockin", "--", "trace", "args"]).unwrap();
        assert_eq!(cli.command, vec!["trace", "args"]);
    }
}
