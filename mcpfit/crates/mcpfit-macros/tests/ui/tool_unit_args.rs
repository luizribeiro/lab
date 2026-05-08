use mcpfit::{Result, Server, tool};

/// Pings the server.
#[tool]
async fn ping(_args: ()) -> Result<&'static str> {
    Ok("pong")
}

fn main() {
    let _ = Server::new("demo", "0.1.0").tool(ping::TOOL);
    assert_eq!(ping::TOOL.name(), "ping");
    assert_eq!(ping::TOOL.description(), Some("Pings the server."));

    let built = ping::TOOL.build();
    let schema = built.input_schema_value().expect("schema set");
    assert_eq!(schema["type"], "object");
    assert!(schema.get("properties").is_none());
}
