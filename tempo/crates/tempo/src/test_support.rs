pub fn happy_sse_body() -> String {
    let frames = [
        r#"{"choices":[{"index":0,"delta":{"role":"assistant"},"finish_reason":null}]}"#,
        r#"{"choices":[{"index":0,"delta":{"content":"Hi"},"finish_reason":null}]}"#,
        r#"{"choices":[{"index":0,"delta":{"content":"!"},"finish_reason":null}]}"#,
        r#"{"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}"#,
        r#"{"choices":[],"usage":{"prompt_tokens":5,"completion_tokens":2,"total_tokens":7}}"#,
        "[DONE]",
    ];
    frames
        .iter()
        .map(|f| format!("data: {f}\n\n"))
        .collect::<String>()
}
