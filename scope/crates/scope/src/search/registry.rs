use std::sync::Arc;

use thiserror::Error;

use super::SearchProvider;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("unknown search provider: {0}")]
    UnknownProvider(String),
    #[error("default search provider not registered: {0}")]
    DefaultMissing(String),
}

pub struct SearchRegistry {
    providers: Vec<Arc<dyn SearchProvider>>,
    default_name: String,
}

impl SearchRegistry {
    pub fn new(default_name: impl Into<String>) -> Self {
        Self {
            providers: Vec::new(),
            default_name: default_name.into(),
        }
    }

    pub fn register(&mut self, provider: Arc<dyn SearchProvider>) {
        self.providers.push(provider);
    }

    pub fn pick(
        &self,
        override_name: Option<&str>,
    ) -> Result<Arc<dyn SearchProvider>, RegistryError> {
        let name = override_name.unwrap_or(&self.default_name);
        self.providers
            .iter()
            .rev()
            .find(|p| p.name() == name)
            .cloned()
            .ok_or_else(|| match override_name {
                Some(n) => RegistryError::UnknownProvider(n.to_string()),
                None => RegistryError::DefaultMissing(name.to_string()),
            })
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;
    use crate::types::{SearchOutput, SearchRequest, SearchResult};

    struct FakeProvider {
        name: &'static str,
        marker: &'static str,
    }

    #[async_trait]
    impl SearchProvider for FakeProvider {
        fn name(&self) -> &str {
            self.name
        }

        async fn search(&self, request: SearchRequest) -> anyhow::Result<SearchOutput> {
            Ok(SearchOutput {
                query: request.query,
                results: vec![SearchResult {
                    title: self.marker.into(),
                    url: "https://example.com/".into(),
                    snippet: None,
                }],
            })
        }
    }

    fn provider(name: &'static str, marker: &'static str) -> Arc<dyn SearchProvider> {
        Arc::new(FakeProvider { name, marker })
    }

    #[test]
    fn default_selected_when_no_override() {
        let mut reg = SearchRegistry::new("ddg");
        reg.register(provider("other", "o"));
        reg.register(provider("ddg", "d"));
        assert_eq!(reg.pick(None).unwrap().name(), "ddg");
    }

    #[test]
    fn explicit_override_wins() {
        let mut reg = SearchRegistry::new("ddg");
        reg.register(provider("ddg", "d"));
        reg.register(provider("other", "o"));
        assert_eq!(reg.pick(Some("other")).unwrap().name(), "other");
    }

    #[test]
    fn unknown_override_errors() {
        let mut reg = SearchRegistry::new("ddg");
        reg.register(provider("ddg", "d"));
        match reg.pick(Some("missing")) {
            Err(RegistryError::UnknownProvider(n)) => assert_eq!(n, "missing"),
            Ok(p) => panic!("expected error, got provider: {}", p.name()),
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn missing_default_errors() {
        let mut reg = SearchRegistry::new("ddg");
        reg.register(provider("other", "o"));
        match reg.pick(None) {
            Err(RegistryError::DefaultMissing(n)) => assert_eq!(n, "ddg"),
            Ok(p) => panic!("expected error, got provider: {}", p.name()),
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[tokio::test]
    async fn duplicate_registration_last_wins() {
        let mut reg = SearchRegistry::new("ddg");
        reg.register(provider("ddg", "first"));
        reg.register(provider("ddg", "second"));
        let picked = reg.pick(None).unwrap();
        let out = picked
            .search(SearchRequest {
                query: "q".into(),
                limit: None,
            })
            .await
            .unwrap();
        assert_eq!(out.results[0].title, "second");
    }
}
