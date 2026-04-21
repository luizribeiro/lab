use std::io::{Read, Write};
use std::net::TcpListener;
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
fn proxy_mode_injects_http_proxy_env_into_child() {
    let config = write_config(
        r#"
        [sandbox.network]
        mode = "proxy"
        allow_hosts = ["example.com"]
        "#,
    );
    let probe = probe_binary();
    let output = run_lockin(&[
        "-c",
        config.path().to_str().unwrap(),
        "--",
        probe.to_str().unwrap(),
        "print-env",
        "HTTP_PROXY",
    ]);
    assert!(
        output.status.success(),
        "probe should read HTTP_PROXY: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.trim().starts_with("http://127.0.0.1:"),
        "HTTP_PROXY should point at a loopback port, got: {stdout:?}"
    );
}

#[test]
fn proxy_mode_denies_non_loopback_tcp_connect() {
    let config = write_config(
        r#"
        [sandbox.network]
        mode = "proxy"
        allow_hosts = ["example.com"]
        "#,
    );
    let probe = probe_binary();
    let output = run_lockin(&[
        "-c",
        config.path().to_str().unwrap(),
        "--",
        probe.to_str().unwrap(),
        "can-connect",
        "1.1.1.1",
        "80",
    ]);
    assert!(
        !output.status.success(),
        "proxy mode must deny non-loopback TCP; exit={:?} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Echo server on `127.0.0.1:0`. Accepts one connection, echoes
/// bytes until the peer closes. Returns its bound port. Used by the
/// proxy-mode E2E test as the target the sandboxed probe talks to
/// through the proxy.
fn start_echo_server() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind echo");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        while let Ok((mut sock, _)) = listener.accept() {
            std::thread::spawn(move || {
                let mut buf = [0u8; 1];
                while let Ok(n) = sock.read(&mut buf) {
                    if n == 0 {
                        return;
                    }
                    if sock.write_all(&buf[..n]).is_err() {
                        return;
                    }
                }
            });
        }
    });
    port
}

#[test]
fn proxy_mode_end_to_end_reaches_allowlisted_host() {
    let echo_port = start_echo_server();
    let config = write_config(
        r#"
        [sandbox.network]
        mode = "proxy"
        allow_hosts = ["localhost"]
        "#,
    );
    let probe = probe_binary();
    let output = run_lockin(&[
        "-c",
        config.path().to_str().unwrap(),
        "--",
        probe.to_str().unwrap(),
        "can-proxy-connect",
        &format!("localhost:{echo_port}"),
    ]);
    assert!(
        output.status.success(),
        "E2E proxy path failed: exit={:?} stdout={} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn proxy_mode_denies_host_not_in_allowlist() {
    let echo_port = start_echo_server();
    let config = write_config(
        r#"
        [sandbox.network]
        mode = "proxy"
        allow_hosts = ["only-me.example"]
        "#,
    );
    let probe = probe_binary();
    let output = run_lockin(&[
        "-c",
        config.path().to_str().unwrap(),
        "--",
        probe.to_str().unwrap(),
        "can-proxy-connect",
        &format!("localhost:{echo_port}"),
    ]);
    assert!(
        !output.status.success(),
        "proxy must reject hosts outside allow_hosts; exit={:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    // Assert on the HTTP status evidence only, not the probe's full
    // error prefix, so the test doesn't break if probe formatting
    // changes.
    assert!(
        stderr.contains("HTTP/1.1 403"),
        "expected 403 status in probe stderr, got: {stderr:?}"
    );
}

#[test]
fn allow_all_mode_does_not_inject_proxy_env() {
    let config = write_config(
        r#"
        [sandbox.network]
        mode = "allow_all"
        "#,
    );
    let probe = probe_binary();
    let output = run_lockin(&[
        "-c",
        config.path().to_str().unwrap(),
        "--",
        probe.to_str().unwrap(),
        "print-env",
        "HTTP_PROXY",
    ]);
    assert!(
        !output.status.success(),
        "allow_all mode must NOT inject HTTP_PROXY; exit={:?}",
        output.status.code()
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("env var `HTTP_PROXY`"),
        "expected probe to report HTTP_PROXY missing; got stderr: {stderr:?}"
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
