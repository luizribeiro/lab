use std::time::Duration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;

use crate::config::ExternalSearchConfig;
use crate::plugin::protocol::{SearchRequest as PluginSearchRequest, SearchResponse, PROTOCOL_NAME};
use crate::plugin::PluginRunner;
use crate::types::{SearchOutput, SearchRequest, SearchResult};

use super::SearchProvider;

pub struct ExternalSearchProvider {
    name: String,
    runner: PluginRunner,
}

impl std::fmt::Debug for ExternalSearchProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalSearchProvider")
            .field("name", &self.name)
            .finish()
    }
}

impl ExternalSearchProvider {
    pub fn from_config(cfg: ExternalSearchConfig, http_timeout: Duration) -> Result<Self> {
        if cfg.protocol != PROTOCOL_NAME {
            return Err(anyhow!(
                "search provider {}: unsupported protocol {}, expected {}",
                cfg.name,
                cfg.protocol,
                PROTOCOL_NAME
            ));
        }
        Ok(Self {
            name: cfg.name,
            runner: PluginRunner::new(cfg.command, http_timeout),
        })
    }
}

#[async_trait]
impl SearchProvider for ExternalSearchProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn search(&self, request: SearchRequest) -> Result<SearchOutput> {
        let req = PluginSearchRequest::new(request.query.clone(), request.limit);
        let response: SearchResponse = self.runner.run(&req).await?;
        if !response.ok {
            return Err(anyhow!(response
                .error
                .unwrap_or_else(|| "external search provider failed".to_string())));
        }
        let results = response
            .results
            .into_iter()
            .map(|r| SearchResult {
                title: r.title,
                url: r.url,
                snippet: r.snippet,
            })
            .collect();
        Ok(SearchOutput {
            query: request.query,
            results,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(protocol: &str) -> ExternalSearchConfig {
        ExternalSearchConfig {
            name: "test".into(),
            command: vec!["true".into()],
            protocol: protocol.into(),
        }
    }

    #[test]
    fn from_config_rejects_bad_protocol() {
        let err = ExternalSearchProvider::from_config(cfg("scope-json-v2"), Duration::from_secs(5))
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("unsupported protocol"), "got: {msg}");
    }

    #[test]
    fn from_config_accepts_default_protocol() {
        ExternalSearchProvider::from_config(cfg(PROTOCOL_NAME), Duration::from_secs(5)).unwrap();
    }

    #[tokio::test]
    async fn search_returns_parsed_results_from_script() {
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let script = dir.path().join("plugin.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\ncat > /dev/null\nprintf '%s' '{\"schema_version\":1,\"ok\":true,\"results\":[{\"title\":\"A\",\"url\":\"https://a.test/\",\"snippet\":\"hi\"},{\"title\":\"B\",\"url\":\"https://b.test/\"}]}'\n",
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();

        let provider = ExternalSearchProvider::from_config(
            ExternalSearchConfig {
                name: "fixture".into(),
                command: vec![script.to_string_lossy().into_owned()],
                protocol: "scope-json-v1".into(),
            },
            Duration::from_secs(5),
        )
        .unwrap();

        let output = provider
            .search(SearchRequest {
                query: "rust".into(),
                limit: Some(2),
            })
            .await
            .unwrap();
        assert_eq!(output.query, "rust");
        assert_eq!(output.results.len(), 2);
        assert_eq!(output.results[0].title, "A");
        assert_eq!(output.results[0].url, "https://a.test/");
        assert_eq!(output.results[0].snippet.as_deref(), Some("hi"));
        assert_eq!(output.results[1].title, "B");
        assert_eq!(output.results[1].snippet, None);
    }

    #[tokio::test]
    async fn search_returns_error_when_plugin_reports_failure() {
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let script = dir.path().join("plugin.sh");
        std::fs::write(
            &script,
            "#!/bin/sh\ncat > /dev/null\nprintf '%s' '{\"schema_version\":1,\"ok\":false,\"error\":\"nope\"}'\n",
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();

        let provider = ExternalSearchProvider::from_config(
            ExternalSearchConfig {
                name: "fixture".into(),
                command: vec![script.to_string_lossy().into_owned()],
                protocol: "scope-json-v1".into(),
            },
            Duration::from_secs(5),
        )
        .unwrap();

        let err = provider
            .search(SearchRequest {
                query: "q".into(),
                limit: None,
            })
            .await
            .unwrap_err();
        assert!(format!("{err:#}").contains("nope"));
    }
}
