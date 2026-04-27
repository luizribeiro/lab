use async_trait::async_trait;

use crate::types::{SearchOutput, SearchRequest};

pub mod registry;

pub use registry::{RegistryError, SearchRegistry};

#[async_trait]
pub trait SearchProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn search(&self, request: SearchRequest) -> anyhow::Result<SearchOutput>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SearchResult;

    struct FakeProvider;

    #[async_trait]
    impl SearchProvider for FakeProvider {
        fn name(&self) -> &str {
            "fake"
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
