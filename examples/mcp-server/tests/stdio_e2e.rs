use std::io::Write;
use std::process::{Child, Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use fittings::serde_json::{self, json, Value};

fn mcp_server_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mcp-server")
}

fn wait_for_child_output_with_timeout(mut child: Child, timeout: Duration) -> Output {
    let start = Instant::now();

    loop {
        if child.try_wait().expect("check child status").is_some() {
            return child.wait_with_output().expect("collect child output");
        }

        if start.elapsed() > timeout {
            child.kill().expect("kill hung child process");
            let output = child.wait_with_output().expect("collect child output");
            panic!(
                "mcp-server did not exit within {timeout:?}; stdout={:?}; stderr={:?}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        thread::sleep(Duration::from_millis(10));
    }
}

fn run_stdio_serve(stdin_payload: &[u8]) -> Output {
    let mut command = Command::new(mcp_server_bin());
    command
        .env("FITTINGS", "1")
        .arg("serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().expect("spawn mcp-server");

    {
        let stdin = child.stdin.as_mut().expect("stdin should be piped");
        stdin
            .write_all(stdin_payload)
            .expect("write payload to child stdin");
    }

    drop(child.stdin.take());

    wait_for_child_output_with_timeout(child, Duration::from_secs(2))
}

fn parse_response_lines(stdout: &[u8]) -> Vec<Value> {
    let stdout_text = std::str::from_utf8(stdout).expect("stdout should be utf-8");
    stdout_text
        .lines()
        .map(|line| {
            serde_json::from_str::<Value>(line).expect("response line should be valid json")
        })
        .collect()
}

fn response_by_id<'a>(responses: &'a [Value], expected_id: &str) -> &'a Value {
    responses
        .iter()
        .find(|response| response["id"] == expected_id)
        .unwrap_or_else(|| panic!("missing response id `{expected_id}`"))
}

fn assert_success_response_envelope(response: &Value, expected_id: Value) {
    let object = response
        .as_object()
        .expect("response should be a json object");

    let mut keys: Vec<_> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["id", "jsonrpc", "result"]);

    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], expected_id);
    assert!(
        response.get("error").is_none(),
        "error field must be absent"
    );
}

fn assert_error_response_envelope(response: &Value, expected_id: Value) {
    let object = response
        .as_object()
        .expect("response should be a json object");

    let mut keys: Vec<_> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, vec!["error", "id", "jsonrpc"]);

    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], expected_id);
    assert!(
        response.get("result").is_none(),
        "result field must be absent"
    );
}

#[test]
fn stdio_e2e_initialize_list_and_call_follow_strict_jsonrpc_envelopes() {
    let payload = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":\"init-1\",\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-01-01\",\"clientInfo\":{\"name\":\"test-client\",\"version\":\"0.1.0\"}}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"list-1\",\"method\":\"tools/list\",\"params\":{}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"call-1\",\"method\":\"tools/call\",\"params\":{\"name\":\"add\",\"arguments\":{\"a\":2,\"b\":3}}}\n"
    );

    let output = run_stdio_serve(payload.as_bytes());

    assert!(output.status.success());
    assert!(output.stderr.is_empty(), "stderr must be empty");

    let responses = parse_response_lines(&output.stdout);
    assert_eq!(responses.len(), 3);

    let initialize = response_by_id(&responses, "init-1");
    assert_success_response_envelope(initialize, json!("init-1"));
    assert_eq!(initialize["result"]["protocolVersion"], "2025-01-01");
    assert_eq!(
        initialize["result"]["capabilities"]["tools"]["listChanged"],
        false
    );
    assert_eq!(
        initialize["result"]["serverInfo"]["name"],
        "fittings-mcp-example"
    );

    let list = response_by_id(&responses, "list-1");
    assert_success_response_envelope(list, json!("list-1"));
    let tools = list["result"]["tools"]
        .as_array()
        .expect("tools/list result should include tools array");
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0]["name"], "add");
    assert_eq!(tools[1]["name"], "echo");

    let call = response_by_id(&responses, "call-1");
    assert_success_response_envelope(call, json!("call-1"));
    assert_eq!(call["result"]["isError"], false);
    assert_eq!(call["result"]["content"][0]["type"], "text");
    assert_eq!(call["result"]["content"][0]["text"], "5");
}

#[test]
fn stdio_e2e_invalid_tool_arguments_return_error_envelope() {
    let request = br#"{"jsonrpc":"2.0","id":"call-bad-1","method":"tools/call","params":{"name":"add","arguments":{"a":"x","b":1}}}
"#;

    let output = run_stdio_serve(request);

    assert!(output.status.success());
    assert!(output.stderr.is_empty(), "stderr must be empty");

    let responses = parse_response_lines(&output.stdout);
    assert_eq!(responses.len(), 1);

    let response = &responses[0];
    assert_error_response_envelope(response, json!("call-bad-1"));
    assert_eq!(response["error"]["code"], -32602);
    assert_eq!(response["error"]["message"], "Invalid params");
}

#[test]
fn stdio_e2e_initialized_notification_is_accepted_without_response_line() {
    let payload = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":\"init-1\",\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-01-01\",\"clientInfo\":{\"name\":\"test-client\",\"version\":\"0.1.0\"}}}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\",\"params\":{}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"list-1\",\"method\":\"tools/list\",\"params\":{}}\n"
    );

    let output = run_stdio_serve(payload.as_bytes());

    assert!(output.status.success());
    assert!(output.stderr.is_empty(), "stderr must be empty");

    let responses = parse_response_lines(&output.stdout);
    assert_eq!(responses.len(), 2, "notification must not emit a response");

    let initialize = response_by_id(&responses, "init-1");
    assert_success_response_envelope(initialize, json!("init-1"));

    let list = response_by_id(&responses, "list-1");
    assert_success_response_envelope(list, json!("list-1"));
}

#[test]
fn stdio_e2e_initialized_request_before_initialize_returns_invalid_request() {
    let request =
        br#"{"jsonrpc":"2.0","id":"early-init","method":"notifications/initialized","params":{}}
"#;

    let output = run_stdio_serve(request);

    assert!(output.status.success());
    assert!(output.stderr.is_empty(), "stderr must be empty");

    let responses = parse_response_lines(&output.stdout);
    assert_eq!(responses.len(), 1);

    let response = &responses[0];
    assert_error_response_envelope(response, json!("early-init"));
    assert_eq!(response["error"]["code"], -32600);
    assert_eq!(response["error"]["message"], "Invalid Request");
}
