use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use scraper::{Html, Selector};
use url::{form_urlencoded, Url};

use crate::http::HttpClient;
use crate::providers::{ProviderInfo, ProviderKind, ProviderSource};
use crate::types::{SearchOutput, SearchRequest, SearchResult};

use super::SearchProvider;

const DEFAULT_ENDPOINT: &str = "https://html.duckduckgo.com/html/";

pub struct DuckDuckGoSearchProvider {
    http: Arc<HttpClient>,
    endpoint: Url,
}

impl DuckDuckGoSearchProvider {
    pub fn new(http: Arc<HttpClient>) -> Self {
        Self {
            http,
            endpoint: Url::parse(DEFAULT_ENDPOINT).expect("default DDG endpoint is valid"),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_endpoint(http: Arc<HttpClient>, endpoint: Url) -> Self {
        Self { http, endpoint }
    }
}

#[async_trait]
impl SearchProvider for DuckDuckGoSearchProvider {
    fn name(&self) -> &str {
        "duckduckgo"
    }

    fn describe(&self) -> ProviderInfo {
        ProviderInfo {
            kind: ProviderKind::Search,
            name: "duckduckgo".into(),
            source: ProviderSource::Builtin,
            summary: "https://duckduckgo.com/".into(),
        }
    }

    async fn search(&self, request: SearchRequest) -> Result<SearchOutput> {
        let mut url = self.endpoint.clone();
        url.query_pairs_mut().append_pair("q", &request.query);

        let response = self
            .http
            .fetch(&url)
            .await
            .map_err(|e| anyhow!("duckduckgo search request failed: {e}"))?;

        let mut results = parse_results(&response.body);
        if let Some(limit) = request.limit {
            results.truncate(limit);
        }

        Ok(SearchOutput {
            query: request.query,
            results,
        })
    }
}

pub fn parse_results(html: &str) -> Vec<SearchResult> {
    let document = Html::parse_document(html);
    let result_selector = Selector::parse("div.result").unwrap();
    let title_selector = Selector::parse("a.result__a").unwrap();
    let snippet_selector = Selector::parse(".result__snippet").unwrap();

    let mut results = Vec::new();
    for node in document.select(&result_selector) {
        let Some(title_el) = node.select(&title_selector).next() else {
            continue;
        };
        let title = title_el.text().collect::<String>().trim().to_string();
        let Some(href) = title_el.value().attr("href") else {
            continue;
        };
        let url = resolve_href(href);

        if title.is_empty() || url.is_empty() {
            continue;
        }

        let snippet = node
            .select(&snippet_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty());

        results.push(SearchResult { title, url, snippet });
    }
    results
}

fn resolve_href(href: &str) -> String {
    let normalized = if let Some(rest) = href.strip_prefix("//") {
        format!("https://{rest}")
    } else {
        href.to_string()
    };

    if let Ok(parsed) = url::Url::parse(&normalized) {
        let host = parsed.host_str().unwrap_or("");
        if host.ends_with("duckduckgo.com") && parsed.path() == "/l/" {
            if let Some((_, value)) = form_urlencoded::parse(parsed.query().unwrap_or("").as_bytes())
                .find(|(k, _)| k == "uddg")
            {
                return value.into_owned();
            }
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_result() {
        let html = r#"
            <html><body>
              <div class="result">
                <a class="result__a" href="https://example.com/page">Example Title</a>
                <a class="result__snippet" href="https://example.com/page">An example snippet.</a>
              </div>
            </body></html>
        "#;
        let results = parse_results(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Example Title");
        assert_eq!(results[0].url, "https://example.com/page");
        assert_eq!(results[0].snippet.as_deref(), Some("An example snippet."));
    }

    #[test]
    fn parses_three_results() {
        let results = parse_results(THREE_RESULT_HTML);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].url, "https://a.example/");
        assert_eq!(results[1].title, "B");
        assert_eq!(results[2].snippet.as_deref(), Some("snippet c"));
    }

    #[test]
    fn empty_results_page_yields_empty_vec() {
        let html = r#"<html><body><p>No results.</p></body></html>"#;
        assert!(parse_results(html).is_empty());
    }

    #[test]
    fn missing_snippet_is_none() {
        let html = r#"
            <div class="result">
              <a class="result__a" href="https://example.com/">Only Title</a>
            </div>
        "#;
        let results = parse_results(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].snippet, None);
    }

    #[test]
    fn unwraps_ddg_redirect_href() {
        let html = r#"
            <div class="result">
              <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Freal.example%2Fpath%3Fq%3D1&rut=abc">Wrapped</a>
              <a class="result__snippet">s</a>
            </div>
        "#;
        let results = parse_results(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://real.example/path?q=1");
    }

    #[test]
    fn skips_results_with_empty_title() {
        let html = r#"
            <div class="result">
              <a class="result__a" href="https://example.com/"></a>
            </div>
        "#;
        assert!(parse_results(html).is_empty());
    }

    use crate::http::test_client;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const THREE_RESULT_HTML: &str = r#"
        <div class="result">
          <a class="result__a" href="https://a.example/">A</a>
          <a class="result__snippet">snippet a</a>
        </div>
        <div class="result">
          <a class="result__a" href="https://b.example/">B</a>
          <a class="result__snippet">snippet b</a>
        </div>
        <div class="result">
          <a class="result__a" href="https://c.example/">C</a>
          <a class="result__snippet">snippet c</a>
        </div>
    "#;

    fn endpoint(server: &MockServer) -> Url {
        Url::parse(&format!("{}/html/", server.uri())).unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn search_passes_query_and_returns_parsed_results() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/html/"))
            .and(query_param("q", "rust lang"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                THREE_RESULT_HTML.as_bytes().to_vec(),
                "text/html; charset=utf-8",
            ))
            .mount(&server)
            .await;

        let provider =
            DuckDuckGoSearchProvider::with_endpoint(test_client(), endpoint(&server));
        assert_eq!(provider.name(), "duckduckgo");

        let output = provider
            .search(SearchRequest {
                query: "rust lang".into(),
                limit: None,
            })
            .await
            .unwrap();

        assert_eq!(output.query, "rust lang");
        assert_eq!(output.results.len(), 3);
        assert_eq!(output.results[0].url, "https://a.example/");
        assert_eq!(output.results[1].title, "B");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn search_truncates_to_limit() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/html/"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                THREE_RESULT_HTML.as_bytes().to_vec(),
                "text/html; charset=utf-8",
            ))
            .mount(&server)
            .await;

        let provider =
            DuckDuckGoSearchProvider::with_endpoint(test_client(), endpoint(&server));
        let output = provider
            .search(SearchRequest {
                query: "rust".into(),
                limit: Some(2),
            })
            .await
            .unwrap();

        assert_eq!(output.results.len(), 2);
        assert_eq!(output.results[0].title, "A");
        assert_eq!(output.results[1].title, "B");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn search_propagates_http_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/html/"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let provider =
            DuckDuckGoSearchProvider::with_endpoint(test_client(), endpoint(&server));
        let err = provider
            .search(SearchRequest {
                query: "rust".into(),
                limit: None,
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("500"), "got: {err}");
    }
}
