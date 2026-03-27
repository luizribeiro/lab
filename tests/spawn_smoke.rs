use std::io::Write;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

fn hello_service_bin() -> &'static str {
    env!("CARGO_BIN_EXE_hello-service")
}

fn run_fittings_command(args: &[&str], stdin_payload: Option<&[u8]>) -> Output {
    let mut command = Command::new(hello_service_bin());
    command
        .env("FITTINGS", "1")
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

#[test]
fn fittings_schema_emits_valid_schema_json() {
    let output = run_fittings_command(&["schema"], None);

    assert!(output.status.success());
    assert!(output.stderr.is_empty(), "stderr must be empty");

    let schema: Value = serde_json::from_slice(&output.stdout).expect("valid schema JSON");
    assert!(schema.is_object());
    assert_eq!(schema["name"], "hello-service");
    assert!(schema["methods"].is_array());
}

#[test]
fn fittings_serve_handles_success_request() {
    let request = br#"{"id":"1","method":"hello","params":{"name":"Ada"},"metadata":{}}
"#;
    let output = run_fittings_command(&["serve"], Some(request));

    assert!(output.status.success(), "serve should exit cleanly");
    assert!(output.stderr.is_empty(), "stderr must be empty");

    let response = parse_single_response(&output.stdout);
    assert_eq!(response["id"], "1");
    assert_eq!(response["result"]["message"], "Hello, Ada!");
    assert!(response.get("error").is_none());
}

#[test]
fn malformed_request_maps_to_parse_error_code() {
    let output = run_fittings_command(&["serve"], Some(b"{bad json\n"));

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
    let output = run_fittings_command(&["serve"], Some(request));

    assert!(output.status.success());
    assert!(output.stderr.is_empty(), "stderr must be empty");

    let response = parse_single_response(&output.stdout);
    assert_eq!(response["id"], "404");
    assert_eq!(response["error"]["code"], -32601);
}

#[test]
fn schema_and_serve_arity_errors_return_usage_and_non_zero_exit() {
    let schema_extra = run_fittings_command(&["schema", "extra"], None);
    assert!(!schema_extra.status.success());
    let schema_stderr = String::from_utf8(schema_extra.stderr).expect("stderr should be utf-8");
    assert!(schema_stderr.contains("Usage:"));
    assert!(schema_extra.stdout.is_empty());

    let serve_extra = run_fittings_command(&["serve", "{}", "extra"], None);
    assert!(!serve_extra.status.success());
    let serve_stderr = String::from_utf8(serve_extra.stderr).expect("stderr should be utf-8");
    assert!(serve_stderr.contains("Usage:"));
    assert!(serve_extra.stdout.is_empty());
}

#[test]
fn partial_eof_is_deterministic_transport_failure() {
    let output = run_fittings_command(&["serve"], Some(b"{\"id\":\"1\""));

    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "protocol stdout must stay empty");

    let stderr = String::from_utf8(output.stderr).expect("stderr should be utf-8");
    assert!(stderr.contains("unexpected end of input before newline"));
}

#[test]
fn oversized_frame_is_deterministic_transport_failure() {
    let mut oversized = vec![b'a'; 1_048_577];
    oversized.push(b'\n');

    let output = run_fittings_command(&["serve"], Some(&oversized));

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
