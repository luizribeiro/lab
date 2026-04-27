use crate::providers::{ProviderInfo, ProviderKind};
use crate::types::{ReadOutput, SearchOutput};

pub fn render_read_output(output: &ReadOutput) -> String {
    let title = output.title.as_deref().unwrap_or(&output.url);
    let url = &output.url;
    let body = &output.markdown;
    format!("# {title}\n\nSource: <{url}>\n\n{body}\n")
}

pub fn render_providers(infos: &[ProviderInfo], default_search: &str) -> String {
    if infos.is_empty() {
        return String::new();
    }
    let kind_w = infos.iter().map(|i| i.kind.label().len()).max().unwrap_or(0);
    let name_w = infos.iter().map(|i| i.name.len()).max().unwrap_or(0);
    let source_w = infos.iter().map(|i| i.source.label().len()).max().unwrap_or(0);
    let mut s = String::new();
    for info in infos {
        let summary = if info.kind == ProviderKind::Search && info.name == default_search {
            if info.summary.is_empty() {
                "(default)".to_string()
            } else {
                format!("{} (default)", info.summary)
            }
        } else {
            info.summary.clone()
        };
        let line = format!(
            "{:<kw$}  {:<nw$}  {:<sw$}  {}",
            info.kind.label(),
            info.name,
            info.source.label(),
            summary,
            kw = kind_w,
            nw = name_w,
            sw = source_w,
        );
        s.push_str(line.trim_end());
        s.push('\n');
    }
    s
}

pub fn render_search_output(output: &SearchOutput) -> String {
    if output.results.is_empty() {
        return format!("No results for `{}`.\n", output.query);
    }
    let mut s = format!("# Search results for `{}`\n\n", output.query);
    for (i, r) in output.results.iter().enumerate() {
        if i > 0 {
            s.push('\n');
        }
        s.push_str(&format!("{}. [{}]({})\n", i + 1, r.title, r.url));
        if let Some(snippet) = &r.snippet {
            s.push_str(&format!("   {snippet}\n"));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SearchResult;

    #[test]
    fn read_renders_title_source_body() {
        let out = ReadOutput {
            url: "https://example.com/post".into(),
            title: Some("Hello".into()),
            markdown: "Body text.".into(),
        };
        assert_eq!(
            render_read_output(&out),
            "# Hello\n\nSource: <https://example.com/post>\n\nBody text.\n"
        );
    }

    #[test]
    fn read_falls_back_to_url_title() {
        let out = ReadOutput {
            url: "https://example.com/x".into(),
            title: None,
            markdown: "x".into(),
        };
        let s = render_read_output(&out);
        assert!(s.starts_with("# https://example.com/x\n"));
        assert!(s.contains("Source: <https://example.com/x>"));
    }

    #[test]
    fn search_renders_numbered_links_with_snippets() {
        let out = SearchOutput {
            query: "rust".into(),
            results: vec![
                SearchResult {
                    title: "Rust".into(),
                    url: "https://rust-lang.org".into(),
                    snippet: Some("A language".into()),
                },
                SearchResult {
                    title: "Crates".into(),
                    url: "https://crates.io".into(),
                    snippet: None,
                },
            ],
        };
        assert_eq!(
            render_search_output(&out),
            "# Search results for `rust`\n\n1. [Rust](https://rust-lang.org)\n   A language\n\n2. [Crates](https://crates.io)\n",
        );
    }

    fn provider(kind: ProviderKind, name: &str, source: crate::providers::ProviderSource, summary: &str) -> ProviderInfo {
        ProviderInfo { kind, name: name.into(), source, summary: summary.into() }
    }

    #[test]
    fn providers_renders_aligned_columns() {
        use crate::providers::ProviderSource;
        let infos = vec![
            provider(ProviderKind::Read, "html", ProviderSource::Builtin, "fallback"),
            provider(ProviderKind::Read, "wikipedia", ProviderSource::External, "host_suffix=wikipedia.org"),
        ];
        let s = render_providers(&infos, "duckduckgo");
        assert_eq!(
            s,
            "read  html       built-in  fallback\nread  wikipedia  external  host_suffix=wikipedia.org\n",
        );
    }

    #[test]
    fn providers_marks_default_search() {
        use crate::providers::ProviderSource;
        let infos = vec![
            provider(ProviderKind::Search, "duckduckgo", ProviderSource::Builtin, "https://duckduckgo.com/"),
            provider(ProviderKind::Search, "wikipedia", ProviderSource::External, ""),
        ];
        let s = render_providers(&infos, "duckduckgo");
        assert!(s.contains("https://duckduckgo.com/ (default)"), "got: {s}");
        assert!(!s.contains("wikipedia external (default)"));
    }

    #[test]
    fn providers_default_with_empty_summary_shows_default_only() {
        use crate::providers::ProviderSource;
        let infos = vec![provider(ProviderKind::Search, "ddg", ProviderSource::Builtin, "")];
        let s = render_providers(&infos, "ddg");
        assert_eq!(s, "search  ddg  built-in  (default)\n");
    }

    #[test]
    fn providers_empty_summary_has_no_trailing_whitespace() {
        use crate::providers::ProviderSource;
        let infos = vec![provider(ProviderKind::Search, "x", ProviderSource::External, "")];
        let s = render_providers(&infos, "ddg");
        assert_eq!(s, "search  x  external\n");
    }

    #[test]
    fn providers_empty_list_returns_empty() {
        assert_eq!(render_providers(&[], "ddg"), "");
    }

    #[test]
    fn search_empty_message() {
        let out = SearchOutput {
            query: "nothing".into(),
            results: vec![],
        };
        assert_eq!(render_search_output(&out), "No results for `nothing`.\n");
    }
}
