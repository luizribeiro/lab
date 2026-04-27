use scraper::{Html, Selector};
use url::form_urlencoded;

use crate::types::SearchResult;

pub fn parse_results(html: &str) -> Vec<SearchResult> {
    let document = Html::parse_document(html);
    let result_selector = Selector::parse("div.result").unwrap();
    let title_selector = Selector::parse("a.result__a").unwrap();
    let snippet_selector = Selector::parse(".result__snippet").unwrap();

    let mut results = Vec::new();
    for node in document.select(&result_selector) {
        let Some(title_el) = node.select(&title_selector).next() else {
            continue;
        };
        let title = title_el.text().collect::<String>().trim().to_string();
        let Some(href) = title_el.value().attr("href") else {
            continue;
        };
        let url = resolve_href(href);

        if title.is_empty() || url.is_empty() {
            continue;
        }

        let snippet = node
            .select(&snippet_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty());

        results.push(SearchResult { title, url, snippet });
    }
    results
}

fn resolve_href(href: &str) -> String {
    let normalized = if let Some(rest) = href.strip_prefix("//") {
        format!("https://{rest}")
    } else {
        href.to_string()
    };

    if let Ok(parsed) = url::Url::parse(&normalized) {
        let host = parsed.host_str().unwrap_or("");
        if host.ends_with("duckduckgo.com") && parsed.path() == "/l/" {
            if let Some((_, value)) = form_urlencoded::parse(parsed.query().unwrap_or("").as_bytes())
                .find(|(k, _)| k == "uddg")
            {
                return value.into_owned();
            }
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_result() {
        let html = r#"
            <html><body>
              <div class="result">
                <a class="result__a" href="https://example.com/page">Example Title</a>
                <a class="result__snippet" href="https://example.com/page">An example snippet.</a>
              </div>
            </body></html>
        "#;
        let results = parse_results(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Example Title");
        assert_eq!(results[0].url, "https://example.com/page");
        assert_eq!(results[0].snippet.as_deref(), Some("An example snippet."));
    }

    #[test]
    fn parses_three_results() {
        let html = r#"
            <div class="result">
              <a class="result__a" href="https://a.example/">A</a>
              <a class="result__snippet">snippet a</a>
            </div>
            <div class="result">
              <a class="result__a" href="https://b.example/">B</a>
              <a class="result__snippet">snippet b</a>
            </div>
            <div class="result">
              <a class="result__a" href="https://c.example/">C</a>
              <a class="result__snippet">snippet c</a>
            </div>
        "#;
        let results = parse_results(html);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].url, "https://a.example/");
        assert_eq!(results[1].title, "B");
        assert_eq!(results[2].snippet.as_deref(), Some("snippet c"));
    }

    #[test]
    fn empty_results_page_yields_empty_vec() {
        let html = r#"<html><body><p>No results.</p></body></html>"#;
        assert!(parse_results(html).is_empty());
    }

    #[test]
    fn missing_snippet_is_none() {
        let html = r#"
            <div class="result">
              <a class="result__a" href="https://example.com/">Only Title</a>
            </div>
        "#;
        let results = parse_results(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].snippet, None);
    }

    #[test]
    fn unwraps_ddg_redirect_href() {
        let html = r#"
            <div class="result">
              <a class="result__a" href="//duckduckgo.com/l/?uddg=https%3A%2F%2Freal.example%2Fpath%3Fq%3D1&rut=abc">Wrapped</a>
              <a class="result__snippet">s</a>
            </div>
        "#;
        let results = parse_results(html);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://real.example/path?q=1");
    }

    #[test]
    fn skips_results_with_empty_title() {
        let html = r#"
            <div class="result">
              <a class="result__a" href="https://example.com/"></a>
            </div>
        "#;
        assert!(parse_results(html).is_empty());
    }
}
