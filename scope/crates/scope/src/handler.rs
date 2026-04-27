use anyhow::{Context, Result};
use url::Url;

use crate::providers::ProviderKind;
use crate::render::{render_providers, render_read_output, render_search_output};
use crate::runtime::Scope;
use crate::types::{ReadOptions, ReadRequest, SearchRequest};

pub async fn run_read(scope: &Scope, url: &str, reader_name: Option<&str>) -> Result<String> {
    let parsed = Url::parse(url).with_context(|| format!("invalid URL: {url}"))?;
    let reader = scope.readers.pick(&parsed, reader_name)?;
    let request = ReadRequest {
        url: parsed.to_string(),
        options: ReadOptions::default(),
    };
    let output = reader.read(request).await?;
    Ok(render_read_output(&output))
}

pub fn run_providers(scope: &Scope, kind: Option<ProviderKind>) -> String {
    render_providers(&scope.list_providers(kind), scope.searches.default_name())
}

pub async fn run_search(
    scope: &Scope,
    query: &str,
    provider_name: Option<&str>,
    limit: Option<usize>,
) -> Result<String> {
    let provider = scope.searches.pick(provider_name)?;
    let request = SearchRequest {
        query: query.to_string(),
        limit,
    };
    let output = provider.search(request).await?;
    Ok(render_search_output(&output))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::config::Config;
    use crate::http::test_client;
    use crate::read::ReaderRegistry;
    use crate::search::duckduckgo::DuckDuckGoSearchProvider;
    use crate::search::SearchRegistry;
    use url::Url;
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
        let out = run_read(&scope, &url, None).await.unwrap();
        assert!(out.starts_with("# Hi\n"), "got: {out}");
        assert!(out.contains(&format!("Source: <{url}>")));
        assert!(out.contains("# Hello"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn read_unknown_reader_override_errors() {
        let scope = Scope::from_config(&Config::default()).unwrap();
        let err = run_read(&scope, "https://example.com", Some("no-such-reader"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no-such-reader"), "got: {err}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn read_invalid_url_errors() {
        let scope = Scope::from_config(&Config::default()).unwrap();
        let err = run_read(&scope, "not a url", None).await.unwrap_err();
        assert!(err.to_string().contains("invalid URL"), "got: {err}");
    }

    const DDG_RESULT_HTML: &str = r#"
        <div class="result">
          <a class="result__a" href="https://a.example/">Alpha</a>
          <a class="result__snippet">snippet a</a>
        </div>
        <div class="result">
          <a class="result__a" href="https://b.example/">Beta</a>
          <a class="result__snippet">snippet b</a>
        </div>
        <div class="result">
          <a class="result__a" href="https://c.example/">Gamma</a>
          <a class="result__snippet">snippet c</a>
        </div>
    "#;

    async fn scope_with_ddg_at(server: &MockServer) -> Scope {
        Mock::given(method("GET"))
            .and(path("/html/"))
            .respond_with(ResponseTemplate::new(200).set_body_raw(
                DDG_RESULT_HTML.as_bytes().to_vec(),
                "text/html; charset=utf-8",
            ))
            .mount(server)
            .await;

        let http = test_client();
        let endpoint = Url::parse(&format!("{}/html/", server.uri())).unwrap();
        let mut searches = SearchRegistry::new("duckduckgo");
        searches.register(Arc::new(DuckDuckGoSearchProvider::with_endpoint(
            http.clone(),
            endpoint,
        )));
        Scope {
            readers: ReaderRegistry::new(),
            searches,
            http,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn search_returns_markdown_for_ddg_response() {
        let server = MockServer::start().await;
        let scope = scope_with_ddg_at(&server).await;
        let out = run_search(&scope, "rust", None, None).await.unwrap();
        assert!(out.starts_with("# Search results for `rust`\n"), "got: {out}");
        assert!(out.contains("1. [Alpha](https://a.example/)"));
        assert!(out.contains("2. [Beta](https://b.example/)"));
        assert!(out.contains("3. [Gamma](https://c.example/)"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn search_unknown_provider_override_errors() {
        let scope = Scope::from_config(&Config::default()).unwrap();
        let err = run_search(&scope, "rust", Some("no-such-provider"), None)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no-such-provider"), "got: {err}");
    }

    #[test]
    fn providers_lists_builtins_and_marks_default() {
        let scope = Scope::from_config(&Config::default()).unwrap();
        let out = run_providers(&scope, None);
        assert!(out.contains("read"), "got: {out}");
        assert!(out.contains("html"));
        assert!(out.contains("built-in"));
        assert!(out.contains("search"));
        assert!(out.contains("duckduckgo"));
        assert!(out.contains("(default)"), "got: {out}");
    }

    #[test]
    fn providers_filter_read_excludes_search() {
        let scope = Scope::from_config(&Config::default()).unwrap();
        let out = run_providers(&scope, Some(ProviderKind::Read));
        assert!(out.contains("html"));
        assert!(!out.contains("duckduckgo"), "got: {out}");
    }

    #[test]
    fn providers_filter_search_excludes_read() {
        let scope = Scope::from_config(&Config::default()).unwrap();
        let out = run_providers(&scope, Some(ProviderKind::Search));
        assert!(out.contains("duckduckgo"));
        assert!(!out.contains(" html "), "got: {out}");
    }

    #[test]
    fn providers_includes_external_with_route_summary() {
        use crate::config::ExternalReaderConfig;
        use crate::route::Route;

        let config = Config {
            readers: vec![ExternalReaderConfig {
                name: "wiki".into(),
                command: vec!["true".into()],
                protocol: "scope-json-v1".into(),
                priority: 100,
                routes: vec![Route {
                    host_suffix: Some("wikipedia.org".into()),
                    path_prefix: Some("/wiki/".into()),
                    ..Default::default()
                }],
            }],
            ..Config::default()
        };
        let scope = Scope::from_config(&config).unwrap();
        let out = run_providers(&scope, Some(ProviderKind::Read));
        assert!(out.contains("wiki"), "got: {out}");
        assert!(out.contains("external"));
        assert!(out.contains("host_suffix=wikipedia.org"));
        assert!(out.contains("path_prefix=/wiki/"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn search_limit_truncates_results() {
        let server = MockServer::start().await;
        let scope = scope_with_ddg_at(&server).await;
        let out = run_search(&scope, "rust", None, Some(2)).await.unwrap();
        assert!(out.contains("1. [Alpha]"));
        assert!(out.contains("2. [Beta]"));
        assert!(!out.contains("Gamma"), "expected truncation, got: {out}");
    }
}
