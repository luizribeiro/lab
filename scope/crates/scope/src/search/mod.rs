use async_trait::async_trait;

use crate::providers::ProviderInfo;
use crate::types::{SearchOutput, SearchRequest};

pub mod duckduckgo;
pub mod external;
pub mod registry;

pub use registry::{RegistryError, SearchRegistry};

#[async_trait]
pub trait SearchProvider: Send + Sync {
    fn name(&self) -> &str;
    fn describe(&self) -> ProviderInfo;
    async fn search(&self, request: SearchRequest) -> anyhow::Result<SearchOutput>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SearchResult;

    use crate::providers::{ProviderKind, ProviderSource};

    struct FakeProvider;

    #[async_trait]
    impl SearchProvider for FakeProvider {
        fn name(&self) -> &str {
            "fake"
        }

        fn describe(&self) -> ProviderInfo {
            ProviderInfo {
                kind: ProviderKind::Search,
                name: "fake".into(),
                source: ProviderSource::Builtin,
                summary: String::new(),
            }
        }

        async fn search(&self, request: SearchRequest) -> anyhow::Result<SearchOutput> {
            Ok(SearchOutput {
                query: request.query,
                results: vec![SearchResult {
                    title: "Fake".into(),
                    url: "https://example.com/".into(),
                    snippet: None,
                }],
            })
        }
    }

    #[tokio::test]
    async fn search_provider_trait_object_dispatch() {
        let provider: Box<dyn SearchProvider> = Box::new(FakeProvider);
        assert_eq!(provider.name(), "fake");

        let output = provider
            .search(SearchRequest {
                query: "rust".into(),
                limit: None,
            })
            .await
            .unwrap();
        assert_eq!(output.query, "rust");
        assert_eq!(output.results.len(), 1);
    }
}
