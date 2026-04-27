use std::sync::{Arc, OnceLock};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use html_to_markdown_rs::convert;
use regex::Regex;
use url::Url;

use crate::http::HttpClient;
use crate::providers::{ProviderInfo, ProviderKind, ProviderSource};
use crate::route::RouteMatch;
use crate::types::{ReadOutput, ReadRequest};

use super::Reader;

pub fn html_to_markdown(html: &str) -> String {
    convert(html, None)
        .ok()
        .and_then(|result| result.content)
        .unwrap_or_default()
}

pub struct HtmlReader {
    http: Arc<HttpClient>,
}

impl HtmlReader {
    pub fn new(http: Arc<HttpClient>) -> Self {
        Self { http }
    }
}

fn title_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?is)<title[^>]*>(.*?)</title>").unwrap())
}

fn extract_title(html: &str) -> Option<String> {
    title_regex()
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .filter(|t| !t.is_empty())
}

fn is_html_content_type(content_type: &str) -> bool {
    let lower = content_type.to_ascii_lowercase();
    lower.contains("text/html") || lower.contains("application/xhtml+xml")
}

#[async_trait]
impl Reader for HtmlReader {
    fn name(&self) -> &str {
        "html"
    }

    fn matches(&self, url: &Url) -> Option<RouteMatch> {
        match url.scheme() {
            "http" | "https" => Some(RouteMatch { priority: 0, specificity: 0 }),
            _ => None,
        }
    }

    fn describe(&self) -> ProviderInfo {
        ProviderInfo {
            kind: ProviderKind::Read,
            name: "html".into(),
            source: ProviderSource::Builtin,
            summary: "fallback for any http/https URL".into(),
        }
    }

    async fn read(&self, request: ReadRequest) -> Result<ReadOutput> {
        let url = Url::parse(&request.url)
            .map_err(|e| anyhow!("invalid URL {:?}: {e}", request.url))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(anyhow!(
                "html reader only supports http/https, got {:?}",
                url.scheme()
            ));
        }

        let response = self.http.fetch(&url).await?;

        if let Some(content_type) = response.content_type.as_deref() {
            if !is_html_content_type(content_type) {
                return Err(anyhow!(
                    "html reader requires HTML content-type, got {content_type:?} for {}",
                    response.url
                ));
            }
        }

        let title = extract_title(&response.body);
        let markdown = html_to_markdown(&response.body);

        Ok(ReadOutput {
            url: response.url.to_string(),
            title,
            markdown,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::HttpConfig;
    use crate::types::ReadOptions;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn http_client() -> Arc<HttpClient> {
        Arc::new(
            HttpClient::from_config(&HttpConfig {
                timeout_secs: 5,
                max_body_bytes: 1_000_000,
                user_agent: "scope-test/1.0".into(),
            })
            .unwrap(),
        )
    }

    #[test]
    fn heading_becomes_atx() {
        assert!(html_to_markdown("<h1>Title</h1>").contains("# Title"));
    }

    #[test]
    fn link_becomes_inline_markdown() {
        assert!(html_to_markdown(r#"<a href="https://x">x</a>"#).contains("[x](https://x)"));
    }

    #[test]
    fn paragraph_keeps_text() {
        assert!(html_to_markdown("<p>hello</p>").contains("hello"));
    }

    #[test]
    fn empty_input_is_empty() {
        assert!(html_to_markdown("").trim().is_empty());
    }

    #[test]
    fn non_http_scheme_does_not_match() {
        let reader = HtmlReader::new(http_client());
        let url = Url::parse("ftp://example.com/").unwrap();
        assert_eq!(reader.matches(&url), None);
    }

    #[test]
    fn http_and_https_match_with_zero_priority() {
        let reader = HtmlReader::new(http_client());
        for raw in ["http://example.com/", "https://example.com/"] {
            let url = Url::parse(raw).unwrap();
            assert_eq!(
                reader.matches(&url),
                Some(RouteMatch { priority: 0, specificity: 0 })
            );
        }
    }

    fn read_request(url: &str) -> ReadRequest {
        ReadRequest {
            url: url.to_string(),
            options: ReadOptions::default(),
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn reads_html_page_into_markdown() {
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

        let reader = HtmlReader::new(http_client());
        let url = format!("{}/page", server.uri());
        let output = reader.read(read_request(&url)).await.unwrap();

        assert_eq!(output.title.as_deref(), Some("Hi"));
        assert_eq!(output.url, url);
        assert!(output.markdown.contains("# Hello"), "got: {:?}", output.markdown);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn rejects_non_html_content_type() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/data"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_raw(b"{}".to_vec(), "application/json"),
            )
            .mount(&server)
            .await;

        let reader = HtmlReader::new(http_client());
        let err = reader
            .read(read_request(&format!("{}/data", server.uri())))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("HTML content-type"), "got: {err}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn read_rejects_non_http_scheme() {
        let reader = HtmlReader::new(http_client());
        let err = reader
            .read(read_request("ftp://example.com/"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("http/https"), "got: {err}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn missing_title_yields_none() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/notitle"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                "<html><body><p>just text</p></body></html>".as_bytes().to_vec(),
                "text/html",
            ))
            .mount(&server)
            .await;

        let reader = HtmlReader::new(http_client());
        let output = reader
            .read(read_request(&format!("{}/notitle", server.uri())))
            .await
            .unwrap();
        assert_eq!(output.title, None);
        assert!(output.markdown.contains("just text"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn accepts_xhtml_content_type() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/xhtml"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                "<html><head><title>X</title></head><body><h1>Y</h1></body></html>"
                    .as_bytes()
                    .to_vec(),
                "application/xhtml+xml",
            ))
            .mount(&server)
            .await;

        let reader = HtmlReader::new(http_client());
        let output = reader
            .read(read_request(&format!("{}/xhtml", server.uri())))
            .await
            .unwrap();
        assert_eq!(output.title.as_deref(), Some("X"));
    }
}
