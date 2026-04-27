use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use url::Url;

use crate::config::ExternalReaderConfig;
use crate::plugin::protocol::{ReaderOptions, ReaderRequest, ReaderResponse, PROTOCOL_NAME};
use crate::plugin::PluginRunner;
use crate::route::{Route, RouteMatch};
use crate::types::{ReadOutput, ReadRequest};

use super::Reader;

pub struct ExternalReader {
    name: String,
    runner: PluginRunner,
    priority: i32,
    routes: Vec<Route>,
}

impl std::fmt::Debug for ExternalReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalReader")
            .field("name", &self.name)
            .field("priority", &self.priority)
            .field("routes", &self.routes)
            .finish()
    }
}

impl ExternalReader {
    pub fn from_config(cfg: ExternalReaderConfig, http_timeout: Duration) -> Result<Self> {
        if cfg.protocol != PROTOCOL_NAME {
            return Err(anyhow!(
                "reader {}: unsupported protocol {}, expected {}",
                cfg.name,
                cfg.protocol,
                PROTOCOL_NAME
            ));
        }
        Ok(Self {
            name: cfg.name,
            runner: PluginRunner::new(cfg.command, http_timeout),
            priority: cfg.priority,
            routes: cfg.routes,
        })
    }
}

#[async_trait]
impl Reader for ExternalReader {
    fn name(&self) -> &str {
        &self.name
    }

    fn matches(&self, url: &Url) -> Option<RouteMatch> {
        self.routes
            .iter()
            .filter_map(|r| r.matches(url))
            .max_by_key(|m| m.specificity)
            .map(|m| RouteMatch {
                priority: m.priority + self.priority,
                specificity: m.specificity,
            })
    }

    async fn read(&self, request: ReadRequest) -> Result<ReadOutput> {
        let req = ReaderRequest::new(
            request.url.clone(),
            ReaderOptions {
                timeout_secs: request.options.timeout_secs,
            },
        );
        let response: ReaderResponse = self.runner.run(&req).await?;
        if !response.ok {
            return Err(anyhow!(response
                .error
                .unwrap_or_else(|| "external reader failed".to_string())));
        }
        Ok(ReadOutput {
            url: response.url.unwrap_or(request.url),
            title: response.title,
            markdown: response.markdown.unwrap_or_default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(protocol: &str) -> ExternalReaderConfig {
        ExternalReaderConfig {
            name: "test".into(),
            command: vec!["true".into()],
            protocol: protocol.into(),
            priority: 50,
            routes: vec![Route {
                host_suffix: Some("example.com".into()),
                ..Default::default()
            }],
        }
    }

    #[test]
    fn from_config_rejects_bad_protocol() {
        let err = ExternalReader::from_config(cfg("scope-json-v2"), Duration::from_secs(5))
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("unsupported protocol"), "got: {msg}");
    }

    #[test]
    fn from_config_accepts_default_protocol() {
        ExternalReader::from_config(cfg(PROTOCOL_NAME), Duration::from_secs(5)).unwrap();
    }

    #[test]
    fn matches_returns_priority_added() {
        let reader =
            ExternalReader::from_config(cfg(PROTOCOL_NAME), Duration::from_secs(5)).unwrap();
        let m = reader
            .matches(&Url::parse("https://api.example.com/x").unwrap())
            .unwrap();
        assert_eq!(m.priority, 50);
        assert_eq!(m.specificity, 1);
        assert!(reader
            .matches(&Url::parse("https://other.org/").unwrap())
            .is_none());
    }
}
