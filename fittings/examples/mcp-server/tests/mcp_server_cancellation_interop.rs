use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use fittings::serde_json::{self, json, Value};
use fittings::{FittingsError, Server, Transport};
use fittings_testkit::{fixtures::request_line, memory_transport::MemoryTransport};
use mcp_server::mcp::{McpService, McpServiceImpl, ToolRegistry, ToolsCallResult};
use tokio::time::timeout;

fn cancel_notification(request_id: i64) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "method": "notifications/cancelled",
        "params": { "requestId": request_id },
    }))
    .expect("cancel notification should serialize");
    bytes.push(b'\n');
    bytes
}

fn initialized_notification() -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
        "params": {},
    }))
    .expect("initialized notification should serialize");
    bytes.push(b'\n');
    bytes
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn cancellation_via_notifications_cancelled_suppresses_tools_call_response() {
    let observed_cancellation = Arc::new(AtomicBool::new(false));

    let mut registry = ToolRegistry::new();
    let observed_for_handler = Arc::clone(&observed_cancellation);
    registry
        .register(
            "long_running_demo",
            "Long-running tool that supports notifications/cancelled",
            json!({ "type": "object", "additionalProperties": false }),
            move |_arguments, context| {
                for _ in 0..100 {
                    if context.is_cancelled() {
                        observed_for_handler.store(true, Ordering::Release);
                        return Err(FittingsError::invalid_request("tool call cancelled"));
                    }
                    thread::sleep(Duration::from_millis(20));
                }
                Ok(ToolsCallResult::text("long running completed"))
            },
        )
        .expect("tool registration should succeed");

    let service = McpServiceImpl::new(registry);
    let (mut client, server_transport) = MemoryTransport::pair(8);
    let server =
        mcp_server::configure_cancellation(Server::new(service.into_service(), server_transport));
    let handle = tokio::spawn(server.serve());

    client
        .send(&request_line(
            "init-1",
            "initialize",
            json!({ "protocolVersion": "2024-11-05" }),
        ))
        .await
        .expect("send initialize");

    let initialize_frame = timeout(Duration::from_secs(2), client.recv())
        .await
        .expect("initialize response should arrive")
        .expect("initialize response transport ok");
    let initialize_response: Value =
        serde_json::from_slice(&initialize_frame).expect("initialize response should parse");
    assert_eq!(initialize_response["id"], "init-1");
    assert!(initialize_response["result"].is_object());

    client
        .send(&initialized_notification())
        .await
        .expect("send notifications/initialized");

    let call_id: i64 = 7;
    client
        .send(&request_line(
            call_id,
            "tools/call",
            json!({ "name": "long_running_demo", "arguments": {} }),
        ))
        .await
        .expect("send tools/call");

    tokio::time::sleep(Duration::from_millis(100)).await;

    client
        .send(&cancel_notification(call_id))
        .await
        .expect("send notifications/cancelled");

    let recv = timeout(Duration::from_millis(800), client.recv()).await;
    assert!(
        recv.is_err(),
        "tools/call response must be suppressed when token fires, got: {recv:?}",
    );

    assert!(
        observed_cancellation.load(Ordering::Acquire),
        "tool handler should have observed cancellation via ctx.is_cancelled()",
    );

    drop(client);
    let result = handle.await.expect("server task join");
    assert!(result.is_ok(), "server should exit cleanly: {result:?}");
}
