//! Embedding API for scope.
//!
//! ```
//! use scope::{Config, Scope};
//!
//! let config = Config::default();
//! let _scope = Scope::from_config(&config).unwrap();
//! ```

pub mod cli;
pub mod config;
pub mod handler;
pub mod http;
pub mod read;
pub mod render;
pub mod route;
pub mod runtime;
pub mod search;
pub mod types;

pub mod plugin;

pub use config::{Config, ExternalReaderConfig, ExternalSearchConfig, HttpConfig};
pub use plugin::PluginRunner;
pub use read::html::HtmlReader;
pub use read::{Reader, ReaderRegistry, RouteMatch};
pub use route::Route;
pub use runtime::Scope;
pub use search::duckduckgo::DuckDuckGoSearchProvider;
pub use search::{SearchProvider, SearchRegistry};
pub use types::{
    OutputFormat, ReadOptions, ReadOutput, ReadRequest, SearchOutput, SearchRequest, SearchResult,
};

pub mod protocol {
    pub use crate::plugin::protocol::{
        PROTOCOL_NAME, ReaderRequest, ReaderResponse, SCHEMA_VERSION, SearchRequest,
        SearchResponse, SearchResponseResult,
    };
}

#[cfg(test)]
mod embedding_tests {
    use super::*;
    use url::Url;

    #[tokio::test]
    async fn embedding_api_supports_picking_builtins() {
        let scope = Scope::from_config(&Config::default()).unwrap();

        let url = Url::parse("https://example.com/").unwrap();
        let reader = scope.readers.pick(&url, None).unwrap();
        assert_eq!(reader.name(), "html");

        let provider = scope.searches.pick(None).unwrap();
        assert_eq!(provider.name(), "duckduckgo");
    }
}
