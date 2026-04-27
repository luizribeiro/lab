use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PROTOCOL_NAME: &str = "scope-json-v1";
pub const SCHEMA_VERSION: u32 = 1;

const READ_KIND: &str = "read";
const SEARCH_KIND: &str = "search";

#[derive(Debug, Clone, PartialEq, Error)]
pub enum ProtocolError {
    #[error("unsupported schema_version {found}, expected {expected}")]
    SchemaVersionMismatch { found: u32, expected: u32 },
}

pub fn check_schema_version(version: u32) -> Result<(), ProtocolError> {
    if version == SCHEMA_VERSION {
        Ok(())
    } else {
        Err(ProtocolError::SchemaVersionMismatch {
            found: version,
            expected: SCHEMA_VERSION,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReaderOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReaderRequest {
    pub schema_version: u32,
    pub kind: String,
    pub url: String,
    pub options: ReaderOptions,
}

impl ReaderRequest {
    pub fn new(url: impl Into<String>, options: ReaderOptions) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            kind: READ_KIND.to_string(),
            url: url.into(),
            options,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReaderResponse {
    pub schema_version: u32,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub markdown: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ReaderResponse {
    pub fn success(title: Option<String>, url: Option<String>, markdown: String) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            ok: true,
            title,
            url,
            markdown: Some(markdown),
            error: None,
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            ok: false,
            title: None,
            url: None,
            markdown: None,
            error: Some(error.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchRequest {
    pub schema_version: u32,
    pub kind: String,
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

impl SearchRequest {
    pub fn new(query: impl Into<String>, limit: Option<usize>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            kind: SEARCH_KIND.to_string(),
            query: query.into(),
            limit,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchResponseResult {
    pub title: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchResponse {
    pub schema_version: u32,
    pub ok: bool,
    #[serde(default)]
    pub results: Vec<SearchResponseResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl SearchResponse {
    pub fn success(results: Vec<SearchResponseResult>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            ok: true,
            results,
            error: None,
        }
    }

    pub fn failure(error: impl Into<String>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            ok: false,
            results: Vec::new(),
            error: Some(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reader_request_constructor_fills_defaults() {
        let req = ReaderRequest::new(
            "https://example.com",
            ReaderOptions {
                timeout_secs: Some(20),
            },
        );
        assert_eq!(req.schema_version, 1);
        assert_eq!(req.kind, "read");
        assert_eq!(req.url, "https://example.com");
        assert_eq!(req.options.timeout_secs, Some(20));
    }

    #[test]
    fn reader_request_round_trip() {
        let req = ReaderRequest::new(
            "https://example.com/page",
            ReaderOptions {
                timeout_secs: Some(15),
            },
        );
        let json = serde_json::to_string(&req).unwrap();
        let parsed: ReaderRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, req);
    }

    #[test]
    fn reader_request_parses_canonical_json() {
        let json = r#"{"schema_version":1,"kind":"read","url":"https://x","options":{"timeout_secs":20}}"#;
        let parsed: ReaderRequest = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.kind, "read");
        assert_eq!(parsed.url, "https://x");
        assert_eq!(parsed.options.timeout_secs, Some(20));
    }

    #[test]
    fn reader_response_success_round_trip() {
        let resp = ReaderResponse::success(
            Some("Title".into()),
            Some("https://example.com".into()),
            "# body".into(),
        );
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ReaderResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, resp);
        assert!(parsed.ok);
    }

    #[test]
    fn reader_response_failure_shape_parses() {
        let json = r#"{"schema_version":1,"ok":false,"error":"boom"}"#;
        let parsed: ReaderResponse = serde_json::from_str(json).unwrap();
        assert!(!parsed.ok);
        assert_eq!(parsed.error.as_deref(), Some("boom"));
        assert!(parsed.markdown.is_none());
        assert!(parsed.title.is_none());
    }

    #[test]
    fn search_request_constructor_fills_defaults() {
        let req = SearchRequest::new("rust async", Some(5));
        assert_eq!(req.schema_version, 1);
        assert_eq!(req.kind, "search");
        assert_eq!(req.query, "rust async");
        assert_eq!(req.limit, Some(5));
    }

    #[test]
    fn search_request_round_trip() {
        let req = SearchRequest::new("query", Some(10));
        let json = serde_json::to_string(&req).unwrap();
        let parsed: SearchRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, req);
    }

    #[test]
    fn search_response_success_round_trip() {
        let resp = SearchResponse::success(vec![SearchResponseResult {
            title: "T".into(),
            url: "https://example.com".into(),
            snippet: Some("s".into()),
        }]);
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: SearchResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, resp);
    }

    #[test]
    fn search_response_failure_shape_parses() {
        let json = r#"{"schema_version":1,"ok":false,"error":"nope"}"#;
        let parsed: SearchResponse = serde_json::from_str(json).unwrap();
        assert!(!parsed.ok);
        assert_eq!(parsed.error.as_deref(), Some("nope"));
        assert!(parsed.results.is_empty());
    }

    #[test]
    fn search_result_snippet_optional() {
        let json = r#"{"title":"T","url":"https://x"}"#;
        let parsed: SearchResponseResult = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.snippet, None);
    }

    #[test]
    fn check_schema_version_accepts_current() {
        assert!(check_schema_version(SCHEMA_VERSION).is_ok());
    }

    #[test]
    fn check_schema_version_rejects_other() {
        let err = check_schema_version(2).unwrap_err();
        assert_eq!(
            err,
            ProtocolError::SchemaVersionMismatch {
                found: 2,
                expected: 1
            }
        );
        assert!(check_schema_version(0).is_err());
    }

    #[test]
    fn reader_request_rejects_unknown_fields() {
        let json = r#"{"schema_version":1,"kind":"read","url":"x","options":{},"extra":1}"#;
        assert!(serde_json::from_str::<ReaderRequest>(json).is_err());
    }

    #[test]
    fn search_request_rejects_unknown_fields() {
        let json = r#"{"schema_version":1,"kind":"search","query":"q","extra":1}"#;
        assert!(serde_json::from_str::<SearchRequest>(json).is_err());
    }
}
