use std::sync::Arc;

use anyhow::Result;

use crate::config::Config;
use crate::http::HttpClient;
use crate::read::html::HtmlReader;
use crate::read::ReaderRegistry;
use crate::search::duckduckgo::DuckDuckGoSearchProvider;
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

        let mut searches = SearchRegistry::new(config.default_search_provider.clone());
        searches.register(Arc::new(DuckDuckGoSearchProvider::new(http.clone())));

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
