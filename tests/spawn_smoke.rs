use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use fittings::{validate_service_schema, FittingsError};
use hello_api::{HelloParams, HelloResult, HelloServiceClient};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct MissingNameParams {}

#[fittings::service]
trait SpawnHelloInvalidParamsClientService {
    async fn hello(
        &self,
        params: MissingNameParams,
    ) -> Result<HelloResult, fittings::FittingsError>;
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct MissingMethodParams {}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
struct MissingMethodResult {
    ok: bool,
}

#[fittings::service]
trait SpawnHelloUnknownMethodClientService {
    async fn missing(
        &self,
        params: MissingMethodParams,
    ) -> Result<MissingMethodResult, fittings::FittingsError>;
}

fn hello_service_bin() -> &'static str {
    env!("CARGO_BIN_EXE_hello-service")
}

fn run_service_command(
    args: &[&str],
    stdin_payload: Option<&[u8]>,
    fittings_env: Option<&str>,
) -> Output {
    let mut command = Command::new(hello_service_bin());
    if let Some(version) = fittings_env {
        command.env("FITTINGS", version);
    }
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().expect("spawn hello-service");

    if let Some(payload) = stdin_payload {
        let stdin = child.stdin.as_mut().expect("stdin should be piped");
        stdin
            .write_all(payload)
            .expect("write test payload to child stdin");
    }

    drop(child.stdin.take());

    child.wait_with_output().expect("collect child output")
}

fn parse_single_response(stdout: &[u8]) -> Value {
    let stdout_text = std::str::from_utf8(stdout).expect("stdout should be utf-8");
    let mut lines = stdout_text.lines();
    let line = lines.next().expect("expected one response line");
    assert!(lines.next().is_none(), "expected exactly one response line");
    serde_json::from_str(line).expect("response line should be valid json")
}

fn wait_for_child_exit(mut child: Child, timeout: Duration) -> Output {
    let start = Instant::now();

    loop {
        if child.try_wait().expect("check child status").is_some() {
            return child.wait_with_output().expect("collect child output");
        }

        if start.elapsed() > timeout {
            let _ = child.kill();
            panic!("child did not exit within {timeout:?}");
        }

        thread::sleep(Duration::from_millis(10));
    }
}

fn connect_with_retry(address: &str, attempts: usize, delay: Duration) -> TcpStream {
    for _ in 0..attempts {
        if let Ok(stream) = TcpStream::connect(address) {
            return stream;
        }
        thread::sleep(delay);
    }

    panic!("failed to connect to {address} after {attempts} attempts");
}

#[tokio::test]
async fn generated_typed_client_spawn_roundtrip_succeeds() {
    let client = HelloServiceClient::spawn(hello_service_bin())
        .await
        .expect("spawned generated client should connect");

    let result = client
        .hello(HelloParams {
            name: "Ada".to_string(),
        })
        .await
        .expect("typed hello call should succeed");

    assert_eq!(result.message, "Hello, Ada!");
}

#[tokio::test]
async fn generated_typed_client_spawn_with_config_roundtrip_succeeds() {
    let client = HelloServiceClient::spawn_with_config(
        hello_service_bin(),
        serde_json::json!({"log_level": "info"}),
    )
    .await
    .expect("spawned generated client should connect with config");

    let result = client
        .hello(HelloParams {
            name: "Grace".to_string(),
        })
        .await
        .expect("typed hello call should succeed");

    assert_eq!(result.message, "Hello, Grace!");
}

#[tokio::test]
async fn generated_typed_client_surfaces_service_side_invalid_params() {
    let client = SpawnHelloInvalidParamsClientServiceClient::spawn(hello_service_bin())
        .await
        .expect("spawned generated client should connect");

    let error = client
        .hello(MissingNameParams {})
        .await
        .expect_err("service should reject params shape");

    assert!(matches!(
        error,
        FittingsError::InvalidParams(message) if message.contains("name")
    ));
}

#[tokio::test]
async fn generated_typed_client_surfaces_unknown_method_errors() {
    let client = SpawnHelloUnknownMethodClientServiceClient::spawn(hello_service_bin())
        .await
        .expect("spawned generated client should connect");

    let error = client
        .missing(MissingMethodParams {})
        .await
        .expect_err("service should reject unknown method");

    assert!(matches!(
        error,
        FittingsError::MethodNotFound(message) if message == "missing"
    ));
}

#[test]
fn fittings_schema_matches_golden_and_is_rfc_compatible() {
    let output = run_service_command(&["schema"], None, None);

    assert!(output.status.success());
    assert!(output.stderr.is_empty(), "stderr must be empty");

    let schema_value: Value = serde_json::from_slice(&output.stdout).expect("valid schema JSON");
    let schema: fittings::ServiceSchema =
        serde_json::from_value(schema_value.clone()).expect("schema should decode");
    validate_service_schema(&schema).expect("schema should satisfy RFC constraints");

    let fixture_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/golden/hello-service-schema.json");
    let fixture_text = std::fs::read_to_string(&fixture_path).unwrap_or_else(|error| {
        panic!(
            "failed to read schema fixture `{}`: {error}",
            fixture_path.display()
        )
    });
    let fixture_value: Value =
        serde_json::from_str(&fixture_text).expect("schema fixture must be valid JSON");

    assert_eq!(schema_value, fixture_value);
}

#[test]
fn fittings_schema_output_does_not_include_normal_mode_banner() {
    let output = run_service_command(&["schema"], None, None);
    assert!(output.status.success());

    let stdout_text = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(!stdout_text.contains("hello-service listening on"));
}

#[test]
fn serve_tcp_transport_serves_one_connection_without_fittings_env() {
    let mut command = Command::new(hello_service_bin());
    command
        .env_remove("FITTINGS")
        .arg("serve")
        .arg("--transport")
        .arg("tcp")
        .arg("--addr")
        .arg("127.0.0.1:0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .expect("spawn hello-service in serve tcp mode");

    let mut listening_line = String::new();
    {
        let stdout = child.stdout.as_mut().expect("stdout should be piped");
        let mut reader = BufReader::new(stdout);
        reader
            .read_line(&mut listening_line)
            .expect("serve tcp mode should print listening line");
        assert!(listening_line.contains("hello-service listening on"));
    }

    let address = listening_line
        .trim_end()
        .strip_prefix("hello-service listening on ")
        .and_then(|rest| rest.strip_suffix(" (single connection)"))
        .expect("listening line should include bind address")
        .to_string();

    let mut stream = connect_with_retry(&address, 30, Duration::from_millis(20));
    stream
        .write_all(
            br#"{"id":"1","method":"hello","params":{"name":"Ada"},"metadata":{}}
"#,
        )
        .expect("write request");

    let mut response_line = String::new();
    {
        let mut reader = BufReader::new(&mut stream);
        reader
            .read_line(&mut response_line)
            .expect("read response line");
    }
    assert!(!response_line.is_empty(), "response line should be present");

    let response: Value =
        serde_json::from_str(response_line.trim_end()).expect("valid json response");
    assert_eq!(response["id"], "1");
    assert_eq!(response["result"]["message"], "Hello, Ada!");

    drop(stream);

    let output = wait_for_child_exit(child, Duration::from_secs(2));
    assert!(
        output.status.success(),
        "serve tcp mode should exit cleanly"
    );

    let remaining_stdout = String::from_utf8(output.stdout).expect("stdout should be utf-8");
    assert!(
        remaining_stdout.trim().is_empty(),
        "no extra stdout expected"
    );
    assert!(output.stderr.is_empty(), "stderr must be empty");
}

#[test]
fn fittings_serve_handles_success_request() {
    let request = br#"{"id":"1","method":"hello","params":{"name":"Ada"},"metadata":{}}
"#;
    let output = run_service_command(&["serve"], Some(request), None);

    assert!(output.status.success(), "serve should exit cleanly");
    assert!(output.stderr.is_empty(), "stderr must be empty");

    let response = parse_single_response(&output.stdout);
    assert_eq!(response["id"], "1");
    assert_eq!(response["result"]["message"], "Hello, Ada!");
    assert!(response.get("error").is_none());
}

#[test]
fn malformed_request_maps_to_parse_error_code() {
    let output = run_service_command(&["serve"], Some(b"{bad json\n"), None);

    assert!(
        output.status.success(),
        "serve should continue through request error"
    );
    assert!(output.stderr.is_empty(), "stderr must be empty");

    let response = parse_single_response(&output.stdout);
    assert_eq!(response["error"]["code"], -32700);
}

#[test]
fn unknown_method_maps_to_method_not_found_code() {
    let request = br#"{"id":"404","method":"nope","params":{},"metadata":{}}
"#;
    let output = run_service_command(&["serve"], Some(request), None);

    assert!(output.status.success());
    assert!(output.stderr.is_empty(), "stderr must be empty");

    let response = parse_single_response(&output.stdout);
    assert_eq!(response["id"], "404");
    assert_eq!(response["error"]["code"], -32601);
}

#[test]
fn schema_and_serve_arity_errors_return_usage_and_non_zero_exit() {
    let schema_extra = run_service_command(&["schema", "extra"], None, None);
    assert!(!schema_extra.status.success());
    let schema_stderr = String::from_utf8(schema_extra.stderr).expect("stderr should be utf-8");
    assert!(schema_stderr.contains("Usage:"));
    assert!(schema_extra.stdout.is_empty());

    let serve_extra = run_service_command(&["serve", "{}", "extra"], None, None);
    assert!(!serve_extra.status.success());
    let serve_stderr = String::from_utf8(serve_extra.stderr).expect("stderr should be utf-8");
    assert!(serve_stderr.contains("Usage:"));
    assert!(serve_extra.stdout.is_empty());
}

#[test]
fn partial_eof_is_deterministic_transport_failure() {
    let output = run_service_command(&["serve"], Some(b"{\"id\":\"1\""), None);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "protocol stdout must stay empty");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("unexpected end of input before newline"));
}

#[test]
fn oversized_frame_is_deterministic_transport_failure() {
    let mut oversized = vec![b'a'; 1_048_577];
    oversized.push(b'\n');

    let output = run_service_command(&["serve"], Some(&oversized), None);

    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "protocol stdout must stay empty");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("frame exceeds max_frame_bytes"));
}

#[test]
fn broken_pipe_is_deterministic_transport_failure() {
    let mut command = Command::new(hello_service_bin());
    command
        .env("FITTINGS", "1")
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().expect("spawn hello-service");

    drop(child.stdout.take());

    {
        let stdin = child.stdin.as_mut().expect("stdin should be piped");
        stdin
            .write_all(
                br#"{"id":"1","method":"ping","params":{},"metadata":{}}
"#,
            )
            .expect("write request");
    }

    drop(child.stdin.take());

    let output = child.wait_with_output().expect("collect child output");
    assert!(!output.status.success());

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    let stderr_lower = stderr.to_ascii_lowercase();
    assert!(stderr_lower.contains("serve failed"));
    assert!(
        stderr_lower.contains("transport")
            || stderr_lower.contains("broken pipe")
            || stderr_lower.contains("os error")
    );
}
