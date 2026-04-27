use anyhow::{Context, Result};
use url::Url;

use crate::render::render_read_output;
use crate::runtime::Scope;
use crate::types::{OutputFormat, ReadOptions, ReadRequest};

pub async fn run_read(
    scope: &Scope,
    url: &str,
    reader_name: Option<&str>,
    format: OutputFormat,
) -> Result<String> {
    let parsed = Url::parse(url).with_context(|| format!("invalid URL: {url}"))?;
    let reader = scope.readers.pick(&parsed, reader_name)?;
    let request = ReadRequest {
        url: parsed.to_string(),
        options: ReadOptions::default(),
    };
    let output = reader.read(request).await?;
    Ok(render_read_output(&output, format))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test(flavor = "multi_thread")]
    async fn read_returns_markdown_for_html_url() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/page"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                "<html><head><title>Hi</title></head><body><h1>Hello</h1></body></html>"
                    .as_bytes()
                    .to_vec(),
                "text/html; charset=utf-8",
            ))
            .mount(&server)
            .await;

        let scope = Scope::from_config(&Config::default()).unwrap();
        let url = format!("{}/page", server.uri());
        let out = run_read(&scope, &url, None, OutputFormat::Markdown)
            .await
            .unwrap();
        assert!(out.starts_with("# Hi\n"), "got: {out}");
        assert!(out.contains(&format!("Source: <{url}>")));
        assert!(out.contains("# Hello"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn read_json_format_is_parseable() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/page"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                "<html><head><title>T</title></head><body><p>body</p></body></html>"
                    .as_bytes()
                    .to_vec(),
                "text/html",
            ))
            .mount(&server)
            .await;

        let scope = Scope::from_config(&Config::default()).unwrap();
        let url = format!("{}/page", server.uri());
        let out = run_read(&scope, &url, None, OutputFormat::Json).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["url"], url);
        assert_eq!(parsed["title"], "T");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn read_unknown_reader_override_errors() {
        let scope = Scope::from_config(&Config::default()).unwrap();
        let err = run_read(
            &scope,
            "https://example.com",
            Some("no-such-reader"),
            OutputFormat::Markdown,
        )
        .await
        .unwrap_err();
        assert!(err.to_string().contains("no-such-reader"), "got: {err}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn read_invalid_url_errors() {
        let scope = Scope::from_config(&Config::default()).unwrap();
        let err = run_read(&scope, "not a url", None, OutputFormat::Markdown)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("invalid URL"), "got: {err}");
    }
}
