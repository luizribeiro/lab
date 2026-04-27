use crate::types::{OutputFormat, ReadOutput, SearchOutput};

pub fn render_read_output(output: &ReadOutput, format: OutputFormat) -> String {
    match format {
        OutputFormat::Markdown => render_read_markdown(output),
        OutputFormat::Json => to_json(output),
    }
}

pub fn render_search_output(output: &SearchOutput, format: OutputFormat) -> String {
    match format {
        OutputFormat::Markdown => render_search_markdown(output),
        OutputFormat::Json => to_json(output),
    }
}

fn render_read_markdown(output: &ReadOutput) -> String {
    let title = output.title.as_deref().unwrap_or(&output.url);
    let url = &output.url;
    let body = &output.markdown;
    format!("# {title}\n\nSource: <{url}>\n\n{body}\n")
}

fn render_search_markdown(output: &SearchOutput) -> String {
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

fn to_json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).expect("serialize")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SearchResult;

    #[test]
    fn read_markdown_renders_title_source_body() {
        let out = ReadOutput {
            url: "https://example.com/post".into(),
            title: Some("Hello".into()),
            markdown: "Body text.".into(),
        };
        let s = render_read_output(&out, OutputFormat::Markdown);
        assert_eq!(
            s,
            "# Hello\n\nSource: <https://example.com/post>\n\nBody text.\n"
        );
    }

    #[test]
    fn read_markdown_falls_back_to_url_title() {
        let out = ReadOutput {
            url: "https://example.com/x".into(),
            title: None,
            markdown: "x".into(),
        };
        let s = render_read_output(&out, OutputFormat::Markdown);
        assert!(s.starts_with("# https://example.com/x\n"));
        assert!(s.contains("Source: <https://example.com/x>"));
    }

    #[test]
    fn search_markdown_renders_numbered_links_with_snippets() {
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
        let s = render_search_output(&out, OutputFormat::Markdown);
        assert_eq!(
            s,
            "# Search results for `rust`\n\n1. [Rust](https://rust-lang.org)\n   A language\n\n2. [Crates](https://crates.io)\n",
        );
    }

    #[test]
    fn search_markdown_empty_message() {
        let out = SearchOutput {
            query: "nothing".into(),
            results: vec![],
        };
        let s = render_search_output(&out, OutputFormat::Markdown);
        assert_eq!(s, "No results for `nothing`.\n");
    }

    #[test]
    fn read_json_is_pretty_printed_and_valid() {
        let read = ReadOutput {
            url: "https://example.com".into(),
            title: Some("T".into()),
            markdown: "body".into(),
        };
        let s = render_read_output(&read, OutputFormat::Json);
        assert!(s.contains('\n'), "expected pretty-printed JSON");
        let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed["url"], "https://example.com");
        assert_eq!(parsed["title"], "T");
        assert_eq!(parsed["markdown"], "body");
    }

    #[test]
    fn search_json_is_pretty_printed_and_valid() {
        let search = SearchOutput {
            query: "q".into(),
            results: vec![SearchResult {
                title: "t".into(),
                url: "https://u.example".into(),
                snippet: Some("s".into()),
            }],
        };
        let s = render_search_output(&search, OutputFormat::Json);
        assert!(s.contains('\n'), "expected pretty-printed JSON");
        let parsed: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(parsed["query"], "q");
        assert_eq!(parsed["results"][0]["title"], "t");
        assert_eq!(parsed["results"][0]["snippet"], "s");
    }
}
