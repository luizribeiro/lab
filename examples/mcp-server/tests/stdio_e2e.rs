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

fn notification_by_method<'a>(frames: &'a [Value], expected_method: &str) -> &'a Value {
    frames
        .iter()
        .find(|frame| frame.get("id").is_none() && frame["method"] == expected_method)
        .unwrap_or_else(|| panic!("missing notification method `{expected_method}`"))
}

fn notification_count(frames: &[Value], method: &str) -> usize {
    frames
        .iter()
        .filter(|frame| frame.get("id").is_none() && frame["method"] == method)
        .count()
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
        true
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
    assert_eq!(tools.len(), 4);
    assert_eq!(tools[0]["name"], "add");
    assert_eq!(tools[1]["name"], "add_with_details");
    assert_eq!(tools[2]["name"], "echo");
    assert_eq!(tools[3]["name"], "long_running_demo");

    let call = response_by_id(&responses, "call-1");
    assert_success_response_envelope(call, json!("call-1"));
    assert_eq!(call["result"]["isError"], false);
    assert_eq!(call["result"]["content"][0]["type"], "text");
    assert_eq!(call["result"]["content"][0]["text"], "5");
}

#[test]
fn stdio_e2e_structured_tool_call_returns_text_and_structured_content() {
    let request = br#"{"jsonrpc":"2.0","id":"call-structured-1","method":"tools/call","params":{"name":"add_with_details","arguments":{"a":2,"b":3}}}
"#;

    let output = run_stdio_serve(request);

    assert!(output.status.success());
    assert!(output.stderr.is_empty(), "stderr must be empty");

    let responses = parse_response_lines(&output.stdout);
    assert_eq!(responses.len(), 1);

    let response = &responses[0];
    assert_success_response_envelope(response, json!("call-structured-1"));
    assert_eq!(response["result"]["content"][0]["type"], "text");
    assert_eq!(response["result"]["content"][0]["text"], "2 + 3 = 5");
    assert_eq!(
        response["result"]["structuredContent"],
        json!({"a": 2.0, "b": 3.0, "sum": 5.0})
    );
    assert_eq!(response["result"]["isError"], false);
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

#[test]
fn stdio_e2e_runtime_registry_mutation_emits_list_changed_and_updates_tools_list() {
    let payload = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":\"init-1\",\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-01-01\",\"clientInfo\":{\"name\":\"test-client\",\"version\":\"0.1.0\"}}}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\",\"params\":{}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"register-1\",\"method\":\"tools/register\",\"params\":{\"name\":\"runtime_tool\",\"description\":\"Runtime tool\",\"responseText\":\"hello runtime\"}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"list-2\",\"method\":\"tools/list\",\"params\":{}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"call-2\",\"method\":\"tools/call\",\"params\":{\"name\":\"runtime_tool\",\"arguments\":{}}}\n"
    );

    let output = run_stdio_serve(payload.as_bytes());

    assert!(output.status.success());
    assert!(output.stderr.is_empty(), "stderr must be empty");

    let frames = parse_response_lines(&output.stdout);
    assert_eq!(frames.len(), 5);

    let register = response_by_id(&frames, "register-1");
    assert_success_response_envelope(register, json!("register-1"));
    assert_eq!(register["result"]["tool"]["name"], "runtime_tool");

    let changed = notification_by_method(&frames, "notifications/tools/list_changed");
    assert_eq!(changed["jsonrpc"], "2.0");
    assert_eq!(changed["params"], json!({}));
    assert_eq!(
        notification_count(&frames, "notifications/tools/list_changed"),
        1,
        "exactly one list_changed notification should be emitted"
    );

    let list = response_by_id(&frames, "list-2");
    assert_success_response_envelope(list, json!("list-2"));
    let names: Vec<_> = list["result"]["tools"]
        .as_array()
        .expect("tools/list result should include tools array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name should be string"))
        .collect();
    assert_eq!(
        names,
        vec![
            "add",
            "add_with_details",
            "echo",
            "long_running_demo",
            "runtime_tool"
        ]
    );

    let call = response_by_id(&frames, "call-2");
    assert_success_response_envelope(call, json!("call-2"));
    assert_eq!(call["result"]["content"][0]["text"], "hello runtime");
}

#[test]
fn stdio_e2e_long_running_tool_call_can_be_cancelled() {
    let payload = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":\"long-1\",\"method\":\"tools/call\",\"params\":{\"name\":\"long_running_demo\",\"arguments\":{}}}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/cancelled\",\"params\":{\"requestId\":\"long-1\",\"reason\":\"client no longer needs this result\"}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"echo-after-cancel\",\"method\":\"tools/call\",\"params\":{\"name\":\"echo\",\"arguments\":{\"message\":\"still responsive\"}}}\n"
    );

    let started = Instant::now();
    let output = run_stdio_serve(payload.as_bytes());

    assert!(output.status.success());
    assert!(output.stderr.is_empty(), "stderr must be empty");

    let frames = parse_response_lines(&output.stdout);

    assert!(
        started.elapsed() < Duration::from_secs(2),
        "cancelled call should terminate quickly"
    );

    assert!(
        frames.iter().all(|frame| frame["id"] != "long-1"),
        "cancelled request should not produce a response"
    );

    let echo = response_by_id(&frames, "echo-after-cancel");
    assert_success_response_envelope(echo, json!("echo-after-cancel"));
    assert_eq!(echo["result"]["content"][0]["text"], "still responsive");
}

#[test]
fn stdio_e2e_duplicate_in_flight_request_ids_are_rejected() {
    let payload = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":\"dup-1\",\"method\":\"tools/call\",\"params\":{\"name\":\"long_running_demo\",\"arguments\":{}}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"dup-1\",\"method\":\"tools/list\",\"params\":{}}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/cancelled\",\"params\":{\"requestId\":\"dup-1\"}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"echo-after-dup\",\"method\":\"tools/call\",\"params\":{\"name\":\"echo\",\"arguments\":{\"message\":\"ok\"}}}\n"
    );

    let output = run_stdio_serve(payload.as_bytes());

    assert!(output.status.success());
    assert!(output.stderr.is_empty(), "stderr must be empty");

    let frames = parse_response_lines(&output.stdout);

    let duplicate = response_by_id(&frames, "dup-1");
    assert_error_response_envelope(duplicate, json!("dup-1"));
    assert_eq!(duplicate["error"]["code"], -32600);

    let echo = response_by_id(&frames, "echo-after-dup");
    assert_success_response_envelope(echo, json!("echo-after-dup"));
    assert_eq!(echo["result"]["content"][0]["text"], "ok");

    assert_eq!(
        frames.iter().filter(|frame| frame["id"] == "dup-1").count(),
        1,
        "request id should map to at most one response"
    );
}

#[test]
fn stdio_e2e_runtime_registry_mutation_errors_do_not_emit_list_changed_notifications() {
    let payload = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":\"init-1\",\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-01-01\",\"clientInfo\":{\"name\":\"test-client\",\"version\":\"0.1.0\"}}}\n",
        "{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\",\"params\":{}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"register-ok\",\"method\":\"tools/register\",\"params\":{\"name\":\"runtime_tool\",\"responseText\":\"hello runtime\"}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"register-empty\",\"method\":\"tools/register\",\"params\":{\"name\":\"  \",\"responseText\":\"x\"}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"register-dup\",\"method\":\"tools/register\",\"params\":{\"name\":\"runtime_tool\",\"responseText\":\"x\"}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"list-1\",\"method\":\"tools/list\",\"params\":{}}\n"
    );

    let output = run_stdio_serve(payload.as_bytes());

    assert!(output.status.success());
    assert!(output.stderr.is_empty(), "stderr must be empty");

    let frames = parse_response_lines(&output.stdout);

    assert_success_response_envelope(response_by_id(&frames, "register-ok"), json!("register-ok"));

    let empty = response_by_id(&frames, "register-empty");
    assert_error_response_envelope(empty, json!("register-empty"));
    assert_eq!(empty["error"]["code"], -32602);

    let duplicate = response_by_id(&frames, "register-dup");
    assert_error_response_envelope(duplicate, json!("register-dup"));
    assert_eq!(duplicate["error"]["code"], -32600);

    assert_eq!(
        notification_count(&frames, "notifications/tools/list_changed"),
        1,
        "only successful registration should emit list_changed"
    );

    let list = response_by_id(&frames, "list-1");
    assert_success_response_envelope(list, json!("list-1"));
    let names: Vec<_> = list["result"]["tools"]
        .as_array()
        .expect("tools/list result should include tools array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("tool name should be string"))
        .collect();
    assert_eq!(
        names,
        vec![
            "add",
            "add_with_details",
            "echo",
            "long_running_demo",
            "runtime_tool"
        ]
    );
}
