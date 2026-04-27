use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ReadOptions {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadRequest {
    pub url: String,
    #[serde(default)]
    pub options: ReadOptions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReadOutput {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub markdown: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchRequest {
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchOutput {
    pub query: String,
    pub results: Vec<SearchResult>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, clap::ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Markdown,
    Json,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T>(value: &T)
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(value).unwrap();
        let back: T = serde_json::from_str(&json).unwrap();
        assert_eq!(&back, value);
    }

    #[test]
    fn read_request_round_trip() {
        round_trip(&ReadRequest {
            url: "https://example.com".into(),
            options: ReadOptions { timeout_secs: Some(20) },
        });
        round_trip(&ReadRequest {
            url: "https://example.com".into(),
            options: ReadOptions::default(),
        });
    }

    #[test]
    fn read_output_round_trip() {
        round_trip(&ReadOutput {
            url: "https://example.com".into(),
            title: Some("Example".into()),
            markdown: "# Hello".into(),
        });
        round_trip(&ReadOutput {
            url: "https://example.com".into(),
            title: None,
            markdown: String::new(),
        });
    }

    #[test]
    fn search_request_round_trip() {
        round_trip(&SearchRequest {
            query: "rust".into(),
            limit: Some(10),
        });
        round_trip(&SearchRequest {
            query: "rust".into(),
            limit: None,
        });
    }

    #[test]
    fn search_result_round_trip() {
        round_trip(&SearchResult {
            title: "Rust".into(),
            url: "https://rust-lang.org".into(),
            snippet: Some("A language".into()),
        });
    }

    #[test]
    fn search_output_round_trip() {
        round_trip(&SearchOutput {
            query: "rust".into(),
            results: vec![SearchResult {
                title: "Rust".into(),
                url: "https://rust-lang.org".into(),
                snippet: None,
            }],
        });
        round_trip(&SearchOutput {
            query: "empty".into(),
            results: vec![],
        });
    }

    #[test]
    fn output_format_round_trip() {
        round_trip(&OutputFormat::Markdown);
        round_trip(&OutputFormat::Json);
    }

    #[test]
    fn output_format_default_is_markdown() {
        assert_eq!(OutputFormat::default(), OutputFormat::Markdown);
    }
}
