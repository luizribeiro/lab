use std::sync::Arc;

use thiserror::Error;
use url::Url;

use super::Reader;
use crate::route::RouteMatch;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("unknown reader: {0}")]
    UnknownReader(String),
    #[error("no reader matched the URL")]
    NoReader,
    #[error("ambiguous reader selection: {}", .0.join(", "))]
    Ambiguous(Vec<String>),
}

#[derive(Default)]
pub struct ReaderRegistry {
    readers: Vec<Arc<dyn Reader>>,
    fallback: Option<Arc<dyn Reader>>,
}

impl ReaderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, reader: Arc<dyn Reader>) {
        self.readers.push(reader);
    }

    pub fn set_fallback(&mut self, reader: Arc<dyn Reader>) {
        self.fallback = Some(reader);
    }

    pub fn pick(
        &self,
        url: &Url,
        override_name: Option<&str>,
    ) -> Result<Arc<dyn Reader>, RegistryError> {
        if let Some(name) = override_name {
            return self
                .readers
                .iter()
                .chain(self.fallback.iter())
                .find(|r| r.name() == name)
                .cloned()
                .ok_or_else(|| RegistryError::UnknownReader(name.to_string()));
        }

        let candidates: Vec<(&Arc<dyn Reader>, RouteMatch)> = self
            .readers
            .iter()
            .filter_map(|r| r.matches(url).map(|m| (r, m)))
            .collect();

        let Some(best_key) = candidates
            .iter()
            .map(|(_, m)| (m.priority, m.specificity))
            .max()
        else {
            return self
                .fallback
                .clone()
                .ok_or(RegistryError::NoReader);
        };

        let top: Vec<&Arc<dyn Reader>> = candidates
            .iter()
            .filter(|(_, m)| (m.priority, m.specificity) == best_key)
            .map(|(r, _)| *r)
            .collect();

        match top.as_slice() {
            [winner] => Ok((*winner).clone()),
            many => Err(RegistryError::Ambiguous(
                many.iter().map(|r| r.name().to_string()).collect(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;
    use crate::types::{ReadOutput, ReadRequest};

    struct FakeReader {
        name: &'static str,
        match_for: Option<RouteMatch>,
    }

    #[async_trait]
    impl Reader for FakeReader {
        fn name(&self) -> &str {
            self.name
        }

        fn matches(&self, _url: &Url) -> Option<RouteMatch> {
            self.match_for
        }

        async fn read(&self, request: ReadRequest) -> anyhow::Result<ReadOutput> {
            Ok(ReadOutput {
                url: request.url,
                title: None,
                markdown: String::new(),
            })
        }
    }

    fn reader(name: &'static str, m: Option<RouteMatch>) -> Arc<dyn Reader> {
        Arc::new(FakeReader { name, match_for: m })
    }

    fn url() -> Url {
        Url::parse("https://example.com/").unwrap()
    }

    #[test]
    fn priority_wins_over_specificity() {
        let mut reg = ReaderRegistry::new();
        reg.register(reader("low", Some(RouteMatch { priority: 0, specificity: 5 })));
        reg.register(reader("high", Some(RouteMatch { priority: 10, specificity: 0 })));
        assert_eq!(reg.pick(&url(), None).unwrap().name(), "high");
    }

    #[test]
    fn specificity_breaks_priority_tie() {
        let mut reg = ReaderRegistry::new();
        reg.register(reader("a", Some(RouteMatch { priority: 5, specificity: 1 })));
        reg.register(reader("b", Some(RouteMatch { priority: 5, specificity: 3 })));
        assert_eq!(reg.pick(&url(), None).unwrap().name(), "b");
    }

    #[test]
    fn override_selects_by_name() {
        let mut reg = ReaderRegistry::new();
        reg.register(reader("a", Some(RouteMatch { priority: 99, specificity: 99 })));
        reg.register(reader("b", None));
        assert_eq!(reg.pick(&url(), Some("b")).unwrap().name(), "b");
    }

    #[test]
    fn unknown_override_errors() {
        let mut reg = ReaderRegistry::new();
        reg.register(reader("a", Some(RouteMatch { priority: 0, specificity: 0 })));
        match reg.pick(&url(), Some("missing")) {
            Err(RegistryError::UnknownReader(n)) => assert_eq!(n, "missing"),
            Ok(r) => panic!("expected error, got reader: {}", r.name()),
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn no_match_falls_back() {
        let mut reg = ReaderRegistry::new();
        reg.register(reader("a", None));
        reg.set_fallback(reader("fallback", None));
        assert_eq!(reg.pick(&url(), None).unwrap().name(), "fallback");
    }

    #[test]
    fn no_match_without_fallback_errors() {
        let mut reg = ReaderRegistry::new();
        reg.register(reader("a", None));
        assert!(matches!(reg.pick(&url(), None), Err(RegistryError::NoReader)));
    }

    #[test]
    fn ambiguous_tie_lists_both_names() {
        let mut reg = ReaderRegistry::new();
        reg.register(reader("a", Some(RouteMatch { priority: 5, specificity: 2 })));
        reg.register(reader("b", Some(RouteMatch { priority: 5, specificity: 2 })));
        reg.register(reader("c", Some(RouteMatch { priority: 1, specificity: 9 })));
        match reg.pick(&url(), None) {
            Err(RegistryError::Ambiguous(names)) => {
                assert!(names.contains(&"a".to_string()));
                assert!(names.contains(&"b".to_string()));
                assert!(!names.contains(&"c".to_string()));
            }
            Ok(r) => panic!("expected error, got reader: {}", r.name()),
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn override_can_select_fallback() {
        let mut reg = ReaderRegistry::new();
        reg.set_fallback(reader("fallback", None));
        assert_eq!(reg.pick(&url(), Some("fallback")).unwrap().name(), "fallback");
    }
}
