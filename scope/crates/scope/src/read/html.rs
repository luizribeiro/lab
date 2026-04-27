use html_to_markdown_rs::convert;

pub fn html_to_markdown(html: &str) -> String {
    convert(html, None)
        .ok()
        .and_then(|result| result.content)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_becomes_atx() {
        let markdown = html_to_markdown("<h1>Title</h1>");
        assert!(markdown.contains("# Title"), "got: {markdown:?}");
    }

    #[test]
    fn link_becomes_inline_markdown() {
        let markdown = html_to_markdown(r#"<a href="https://x">x</a>"#);
        assert!(markdown.contains("[x](https://x)"), "got: {markdown:?}");
    }

    #[test]
    fn paragraph_keeps_text() {
        let markdown = html_to_markdown("<p>hello</p>");
        assert!(markdown.contains("hello"), "got: {markdown:?}");
    }

    #[test]
    fn empty_input_is_empty() {
        let markdown = html_to_markdown("");
        assert!(markdown.trim().is_empty(), "got: {markdown:?}");
    }
}
