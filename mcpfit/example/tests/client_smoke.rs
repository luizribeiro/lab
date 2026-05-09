use std::time::Duration;

use mcpfit::{Client, ToolContent};
use serde_json::json;
use tokio::time::timeout;

fn mcp_server_bin() -> &'static str {
    env!("CARGO_BIN_EXE_mcp-server")
}

#[tokio::test]
async fn client_smoke_spawn_lists_tools_and_calls_echo() {
    let result = timeout(Duration::from_secs(5), async {
        let client = Client::spawn(mcp_server_bin())
            .await
            .expect("spawn example server");

        let tools = client.list_tools().await.expect("list_tools");
        let names: Vec<&str> = tools.iter().map(|tool| tool.name.as_str()).collect();
        for expected in ["add", "echo"] {
            assert!(
                names.contains(&expected),
                "missing tool `{expected}` in {names:?}"
            );
        }

        let response = client
            .call_tool("echo", json!({ "message": "hello smoke" }))
            .await
            .expect("call_tool echo");
        assert!(
            !response.is_error,
            "echo call returned error: {:?}",
            response.content
        );
        match response.content.first().expect("echo content") {
            ToolContent::Text { text, .. } => assert_eq!(text, "hello smoke"),
            other => panic!("unexpected content: {other:?}"),
        }

        let sum = client
            .call_tool("add", json!({ "a": 2, "b": 3 }))
            .await
            .expect("call_tool add");
        match sum.content.first().expect("add content") {
            ToolContent::Text { text, .. } => assert_eq!(text, "5"),
            other => panic!("unexpected content: {other:?}"),
        }
    })
    .await;

    result.expect("client smoke test should complete within timeout");
}
