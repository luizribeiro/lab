use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;

use crate::config::Config;
use crate::http::HttpClient;
use crate::read::external::ExternalReader;
use crate::read::html::HtmlReader;
use crate::read::ReaderRegistry;
use crate::search::duckduckgo::DuckDuckGoSearchProvider;
use crate::search::external::ExternalSearchProvider;
use crate::search::SearchRegistry;

pub struct Scope {
    pub readers: ReaderRegistry,
    pub searches: SearchRegistry,
    pub http: Arc<HttpClient>,
}

impl Scope {
    pub fn from_config(config: &Config) -> Result<Self> {
        let http = Arc::new(HttpClient::from_config(&config.http)?);

        let html_reader = Arc::new(HtmlReader::new(http.clone()));
        let mut readers = ReaderRegistry::new();
        readers.register(html_reader.clone());
        readers.set_fallback(html_reader);

        let plugin_timeout = Duration::from_secs(config.http.timeout_secs);
        for cfg in &config.readers {
            let external = ExternalReader::from_config(cfg.clone(), plugin_timeout)?;
            readers.register(Arc::new(external));
        }

        let mut searches = SearchRegistry::new(config.default_search_provider.clone());
        searches.register(Arc::new(DuckDuckGoSearchProvider::new(http.clone())));
        for cfg in &config.search_providers {
            let external = ExternalSearchProvider::from_config(cfg.clone(), plugin_timeout)?;
            searches.register(Arc::new(external));
        }

        Ok(Self {
            readers,
            searches,
            http,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::RegistryError as SearchRegistryError;
    use url::Url;

    #[test]
    fn default_config_builds() {
        Scope::from_config(&Config::default()).unwrap();
    }

    #[test]
    fn html_reader_handles_https_urls() {
        let scope = Scope::from_config(&Config::default()).unwrap();
        let url = Url::parse("https://example.com/").unwrap();
        let reader = scope.readers.pick(&url, None).unwrap();
        assert_eq!(reader.name(), "html");
    }

    #[test]
    fn default_search_provider_is_duckduckgo() {
        let scope = Scope::from_config(&Config::default()).unwrap();
        assert_eq!(scope.searches.pick(None).unwrap().name(), "duckduckgo");
    }

    #[tokio::test]
    async fn external_reader_handles_matching_url() {
        use crate::config::ExternalReaderConfig;
        use crate::route::Route;
        use crate::types::{ReadOptions, ReadRequest};
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let script = dir.path().join("plugin.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\ncat > /dev/null\nprintf '%s' '{\"schema_version\":1,\"ok\":true,\"title\":\"From Plugin\",\"url\":\"https://plugin.test/x\",\"markdown\":\"# plugin\"}'\n",
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();

        let config = Config {
            readers: vec![ExternalReaderConfig {
                name: "fixture".into(),
                command: vec![script.to_string_lossy().into_owned()],
                protocol: "scope-json-v1".into(),
                priority: 100,
                routes: vec![Route {
                    host_suffix: Some("plugin.test".into()),
                    ..Default::default()
                }],
            }],
            ..Config::default()
        };
        let scope = Scope::from_config(&config).unwrap();

        let url = Url::parse("https://plugin.test/x").unwrap();
        let reader = scope.readers.pick(&url, None).unwrap();
        assert_eq!(reader.name(), "fixture");

        let output = reader
            .read(ReadRequest {
                url: "https://plugin.test/x".into(),
                options: ReadOptions::default(),
            })
            .await
            .unwrap();
        assert_eq!(output.title.as_deref(), Some("From Plugin"));
        assert_eq!(output.markdown, "# plugin");
        assert_eq!(output.url, "https://plugin.test/x");
    }

    #[test]
    fn external_reader_with_bad_protocol_fails_to_build() {
        use crate::config::ExternalReaderConfig;

        let config = Config {
            readers: vec![ExternalReaderConfig {
                name: "x".into(),
                command: vec!["true".into()],
                protocol: "scope-json-v2".into(),
                priority: 50,
                routes: vec![],
            }],
            ..Config::default()
        };
        assert!(Scope::from_config(&config).is_err());
    }

    #[tokio::test]
    async fn external_search_provider_selected_via_override() {
        use crate::config::ExternalSearchConfig;
        use crate::types::SearchRequest;
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let script = dir.path().join("plugin.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\ncat > /dev/null\nprintf '%s' '{\"schema_version\":1,\"ok\":true,\"results\":[{\"title\":\"X\",\"url\":\"https://x.test/\"}]}'\n",
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();

        let config = Config {
            search_providers: vec![ExternalSearchConfig {
                name: "fixture".into(),
                command: vec![script.to_string_lossy().into_owned()],
                protocol: "scope-json-v1".into(),
            }],
            ..Config::default()
        };
        let scope = Scope::from_config(&config).unwrap();
        let provider = scope.searches.pick(Some("fixture")).unwrap();
        assert_eq!(provider.name(), "fixture");
        let out = provider
            .search(SearchRequest {
                query: "q".into(),
                limit: None,
            })
            .await
            .unwrap();
        assert_eq!(out.results.len(), 1);
        assert_eq!(out.results[0].title, "X");
    }

    #[test]
    fn external_search_provider_with_bad_protocol_fails_to_build() {
        use crate::config::ExternalSearchConfig;

        let config = Config {
            search_providers: vec![ExternalSearchConfig {
                name: "x".into(),
                command: vec!["true".into()],
                protocol: "scope-json-v2".into(),
            }],
            ..Config::default()
        };
        assert!(Scope::from_config(&config).is_err());
    }

    #[test]
    fn configured_default_name_is_honored() {
        let config = Config {
            default_search_provider: "nonexistent".into(),
            ..Config::default()
        };
        let scope = Scope::from_config(&config).unwrap();
        match scope.searches.pick(None) {
            Err(SearchRegistryError::DefaultMissing(n)) => assert_eq!(n, "nonexistent"),
            Ok(p) => panic!("expected DefaultMissing, got: {}", p.name()),
            Err(e) => panic!("unexpected error: {e}"),
        }
        assert_eq!(
            scope.searches.pick(Some("duckduckgo")).unwrap().name(),
            "duckduckgo"
        );
    }
}
