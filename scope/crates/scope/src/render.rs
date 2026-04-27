use crate::types::{ReadOutput, SearchOutput};

pub fn render_read_output(output: &ReadOutput) -> String {
    let title = output.title.as_deref().unwrap_or(&output.url);
    let url = &output.url;
    let body = &output.markdown;
    format!("# {title}\n\nSource: <{url}>\n\n{body}\n")
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

    #[test]
    fn search_empty_message() {
        let out = SearchOutput {
            query: "nothing".into(),
            results: vec![],
        };
        assert_eq!(render_search_output(&out), "No results for `nothing`.\n");
    }
}
