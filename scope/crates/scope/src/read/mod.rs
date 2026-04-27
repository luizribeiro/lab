use async_trait::async_trait;
use url::Url;

use crate::types::{ReadOutput, ReadRequest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteMatch {
    pub priority: i32,
    pub specificity: u32,
}

#[async_trait]
pub trait Reader: Send + Sync {
    fn name(&self) -> &str;
    fn matches(&self, url: &Url) -> Option<RouteMatch>;
    async fn read(&self, request: ReadRequest) -> anyhow::Result<ReadOutput>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeReader;

    #[async_trait]
    impl Reader for FakeReader {
        fn name(&self) -> &str {
            "fake"
        }

        fn matches(&self, _url: &Url) -> Option<RouteMatch> {
            Some(RouteMatch { priority: 0, specificity: 0 })
        }

        async fn read(&self, request: ReadRequest) -> anyhow::Result<ReadOutput> {
            Ok(ReadOutput {
                url: request.url,
                title: Some("fake".into()),
                markdown: "# fake".into(),
            })
        }
    }

    #[tokio::test]
    async fn reader_trait_object_dispatch() {
        let reader: Box<dyn Reader> = Box::new(FakeReader);
        assert_eq!(reader.name(), "fake");

        let url = Url::parse("https://example.com/").unwrap();
        assert_eq!(
            reader.matches(&url),
            Some(RouteMatch { priority: 0, specificity: 0 })
        );

        let output = reader
            .read(ReadRequest {
                url: "https://example.com/".into(),
                options: Default::default(),
            })
            .await
            .unwrap();
        assert_eq!(output.url, "https://example.com/");
        assert_eq!(output.title.as_deref(), Some("fake"));
    }
}
