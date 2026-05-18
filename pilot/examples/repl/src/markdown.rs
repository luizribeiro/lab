use ratatui::text::{Line, Span};
use ratskin::RatSkin;

/// Thin wrapper around `ratskin::RatSkin` that gives us cached, owned
/// `Vec<Line<'static>>` we can hand off to ratatui paragraphs without
/// lifetime gymnastics.
pub struct MarkdownSkin {
    skin: RatSkin,
}

impl MarkdownSkin {
    pub fn new() -> Self {
        Self {
            skin: RatSkin::default(),
        }
    }

    pub fn render(&self, text: &str, width: u16) -> Vec<Line<'static>> {
        let parsed = RatSkin::parse_text(text);
        self.skin
            .parse(parsed, width)
            .into_iter()
            .map(line_into_static)
            .collect()
    }
}

fn line_into_static(line: Line<'_>) -> Line<'static> {
    let spans: Vec<Span<'static>> = line
        .spans
        .into_iter()
        .map(|s| Span::styled(s.content.into_owned(), s.style))
        .collect();
    Line::from(spans).style(line.style)
}
