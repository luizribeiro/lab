use std::path::PathBuf;
use std::process::{Command, Output};

fn lockin_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_lockin"))
}

fn run_lockin(args: &[&str]) -> Output {
    Command::new(lockin_binary())
        .args(args)
        .output()
        .expect("failed to execute lockin")
}

fn write_config(content: &str) -> tempfile::NamedTempFile {
    let tmp = tempfile::Builder::new().suffix(".toml").tempfile().unwrap();
    std::fs::write(tmp.path(), content).unwrap();
    tmp
}

fn probe_binary() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("../../target/debug/sandbox_probe");
    assert!(
        path.exists(),
        "sandbox_probe not found — run `cargo build` first"
    );
    path.canonicalize().unwrap()
}

#[test]
fn run_probe_succeeds() {
    let config = write_config(
        r#"
        [filesystem]
        library_paths_from_env = true
        "#,
    );
    let probe = probe_binary();
    let output = run_lockin(&[
        "-c",
        config.path().to_str().unwrap(),
        "--",
        probe.to_str().unwrap(),
        "can-write-temp",
    ]);
    assert!(
        output.status.success(),
        "lockin failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn exit_code_passthrough() {
    let config = write_config(
        r#"
        [filesystem]
        library_paths_from_env = true
        "#,
    );
    let probe = probe_binary();
    let output = run_lockin(&[
        "-c",
        config.path().to_str().unwrap(),
        "--",
        probe.to_str().unwrap(),
        "can-read",
        "/nonexistent/path",
    ]);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit 1 for failed probe"
    );
}

#[test]
fn missing_config_exits_125() {
    let output = run_lockin(&["-c", "/nonexistent/lockin.toml", "--", "/usr/bin/env"]);
    assert_eq!(output.status.code(), Some(125));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to read config file"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn no_command_exits_125() {
    let output = run_lockin(&[]);
    assert_eq!(output.status.code(), Some(125));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no command specified"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn invalid_config_exits_125() {
    let config = write_config("not_valid { toml");
    let output = run_lockin(&["-c", config.path().to_str().unwrap(), "--", "/usr/bin/env"]);
    assert_eq!(output.status.code(), Some(125));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("failed to parse config file"),
        "unexpected stderr: {stderr}"
    );
}
