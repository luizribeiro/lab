use std::io::{Read, Write};
use std::net::{TcpListener, UdpSocket};
use std::os::unix::net::UnixListener;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

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
    assert!(
        !content.contains("[filesystem]"),
        "test write_config helper appends its own [filesystem] block; \
         passing a [filesystem] section would create a duplicate-table TOML error"
    );
    let tmp = tempfile::Builder::new().suffix(".toml").tempfile().unwrap();
    let suffix = test_exec_dirs_suffix();
    std::fs::write(tmp.path(), format!("{content}\n{suffix}")).unwrap();
    tmp
}

fn test_exec_dirs_suffix() -> String {
    let Some(val) = std::env::var_os("LOCKIN_TEST_EXEC_DIRS") else {
        return String::new();
    };
    let dirs: Vec<String> = std::env::split_paths(&val)
        .filter(|p| !p.as_os_str().is_empty() && p.is_absolute())
        .map(|p| toml_string_literal(&p.to_string_lossy()))
        .collect();
    if dirs.is_empty() {
        return String::new();
    }
    format!("[filesystem]\nexec_dirs = [{}]\n", dirs.join(", "))
}

fn toml_string_literal(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
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
fn env_set_dynamic_linker_var_does_not_reach_child() {
    let config = write_config(
        r#"
        [env]
        inherit = true
        set = { LD_PRELOAD = "/tmp/lockin-test-evil.so", DYLD_INSERT_LIBRARIES = "/tmp/lockin-test-evil.dylib" }
        "#,
    );
    let probe = probe_binary();
    for var in ["LD_PRELOAD", "DYLD_INSERT_LIBRARIES"] {
        let output = run_lockin(&[
            "-c",
            config.path().to_str().unwrap(),
            "--",
            probe.to_str().unwrap(),
            "env-var-unset",
            var,
        ]);
        assert!(
            output.status.success(),
            "{var} from [env].set must not reach child: stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
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

/// In proxy mode the rendered Seatbelt rule is
/// `(allow network-outbound (remote ip "localhost:<proxy-port>"))`.
/// This test pins down that the matcher is exact-port: a direct
/// connect to a *different* loopback port must be denied. Without
/// this assertion a liberal interpretation of the rule (e.g. one that
/// matched any loopback port) would silently allow direct egress to
/// arbitrary loopback services.
#[test]
fn proxy_mode_denies_direct_loopback_tcp_to_wrong_port() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind side-channel listener");
    let wrong_port = listener.local_addr().expect("addr").port();

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
        "127.0.0.1",
        &wrong_port.to_string(),
    ]);
    assert!(
        !output.status.success(),
        "proxy mode must deny direct TCP to non-proxy loopback ports; \
         exit={:?} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// IPv6 loopback (`::1`) is a separate address family from `127.0.0.1`.
/// The proxy listens on IPv4 only, so a direct IPv6 connect must be
/// denied — otherwise proxy mode silently leaks all IPv6 traffic.
#[test]
fn proxy_mode_denies_direct_ipv6_loopback() {
    let Ok(listener) = TcpListener::bind("[::1]:0") else {
        eprintln!("skip: host has no IPv6 loopback");
        return;
    };
    let port = listener.local_addr().expect("addr").port();

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
        "::1",
        &port.to_string(),
    ]);
    assert!(
        !output.status.success(),
        "proxy mode must deny direct IPv6 loopback; exit={:?} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Proxy mode forwards HTTP — i.e. TCP. UDP datagrams must not be
/// allowed to escape via the proxy port (or any other), or proxy
/// mode would leak DNS/UDP traffic outside the HTTP-allowlist policy.
#[test]
fn proxy_mode_denies_udp() {
    let host_sock = UdpSocket::bind("127.0.0.1:0").expect("bind host udp");
    let port = host_sock.local_addr().expect("addr").port();

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
        "can-udp-send",
        "127.0.0.1",
        &port.to_string(),
    ]);
    assert!(
        !output.status.success(),
        "proxy mode must deny UDP egress; exit={:?} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// outpost-proxy is HTTP-over-TCP only; it does not expose an
/// AF_UNIX endpoint. An AF_UNIX outbound from inside proxy mode must
/// therefore be denied, just as it is in deny mode. This guards
/// against a future change to the proxy rendering accidentally
/// granting `(allow network-outbound (literal "..."))` patterns
/// alongside the IP allow.
#[test]
fn proxy_mode_denies_unix_socket_to_outside_path() {
    let temp = tempfile::Builder::new()
        .prefix("lockin-proxy-unix-")
        .tempdir()
        .expect("mkdir tempdir");
    let sock_path = temp.path().join("listener.sock");
    let _listener = UnixListener::bind(&sock_path).expect("bind unix listener");

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
        "can-unix-stream-connect",
        sock_path.to_str().unwrap(),
    ]);
    assert!(
        !output.status.success(),
        "proxy mode must deny AF_UNIX outbound; exit={:?} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Network mode is about *outbound* connectivity through the proxy.
/// `bind(2)` + `listen(2)` on a loopback port is inbound and is not
/// part of the proxy contract — it must be denied so a sandboxed
/// program can't stand up its own listener and accept inbound
/// connections from siblings on the host.
#[test]
fn proxy_mode_denies_loopback_listen() {
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
        "can-tcp-listen",
        "127.0.0.1",
        "0",
    ]);
    assert!(
        !output.status.success(),
        "proxy mode must deny inbound bind/listen; exit={:?} stderr={}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
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

/// Regression test for issue #10: lockin must forward SIGTERM/SIGINT
/// to the sandboxed child's process group and exit with a child-derived
/// numeric status, rather than being killed by the signal's default
/// disposition (which would orphan the child to PID 1).
///
/// The assertion `status.code().is_some() && status.signal().is_none()`
/// is what distinguishes the two outcomes: lockin's supervisor waits
/// for the child and converts the child's signal-termination into a
/// `128 + signal` numeric exit code, leaving lockin itself cleanly
/// exited.
#[test]
fn sigterm_forwarded_to_child_then_lockin_exits_normally() {
    let probe = probe_binary();
    // Tight grace so the test's SIGKILL escalation (needed because
    // syd traps SIGTERM under ptrace) completes within seconds. The
    // production default is 30s — see DEFAULT_SHUTDOWN_GRACE.
    let config = write_config("");
    let mut child = Command::new(lockin_binary())
        .env("LOCKIN_SHUTDOWN_GRACE_MS", "500")
        .args([
            "-c",
            config.path().to_str().unwrap(),
            "--",
            probe.to_str().unwrap(),
            "pause",
            "30",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn lockin");

    wait_for_descendant_process(child.id() as i32, Duration::from_secs(5));

    // SAFETY: kill() with a valid pid and signal number is sound.
    let lockin_pid = child.id() as i32;
    assert_eq!(
        unsafe { libc::kill(lockin_pid, libc::SIGTERM) },
        0,
        "SIGTERM lockin: {}",
        std::io::Error::last_os_error()
    );

    let status = match wait_with_timeout(&mut child, Duration::from_secs(10)) {
        Some(s) => s,
        None => {
            let _ = child.kill();
            let mut stderr = String::new();
            if let Some(mut e) = child.stderr.take() {
                let _ = e.read_to_string(&mut stderr);
            }
            panic!("lockin did not exit within 10s of SIGTERM; stderr=\n{stderr}");
        }
    };

    assert!(
        status.code().is_some() && status.signal().is_none(),
        "lockin must return a child-derived exit code after forwarding SIGTERM, \
         not be killed by the signal itself; got status.signal()={:?}, code={:?}",
        status.signal(),
        status.code()
    );
}

/// Polls `/proc/<lockin_pid>/task/.../children` (Linux) or `pgrep`
/// (macOS) until the lockin parent has at least one descendant, so
/// the test doesn't race the wrapper's spawn of the probe.
fn wait_for_descendant_process(lockin_pid: i32, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if has_descendant(lockin_pid) {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("lockin pid {lockin_pid} never spawned a descendant within {timeout:?}");
}

#[cfg(target_os = "linux")]
fn has_descendant(lockin_pid: i32) -> bool {
    let task_dir = format!("/proc/{lockin_pid}/task");
    let Ok(entries) = std::fs::read_dir(&task_dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path().join("children");
        if let Ok(contents) = std::fs::read_to_string(&path) {
            if !contents.trim().is_empty() {
                return true;
            }
        }
    }
    false
}

#[cfg(target_os = "macos")]
fn has_descendant(lockin_pid: i32) -> bool {
    Command::new("pgrep")
        .args(["-P", &lockin_pid.to_string()])
        .output()
        .map(|o| o.status.success() && !o.stdout.is_empty())
        .unwrap_or(false)
}

fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => return None,
        }
    }
    None
}

#[test]
fn bare_argv0_from_cli_rejected_with_125() {
    let output = run_lockin(&["--", "python3", "--help"]);
    assert_eq!(output.status.code(), Some(125));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("explicit executable path") && stderr.contains("Bare PATH"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn bare_argv0_from_config_rejected_with_125() {
    let config = write_config(
        r#"
        command = ["python3"]
        "#,
    );
    let output = run_lockin(&["-c", config.path().to_str().unwrap()]);
    assert_eq!(output.status.code(), Some(125));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("explicit executable path"),
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
