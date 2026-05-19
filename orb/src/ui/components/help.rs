//! `/help` modal — a read-only listing of every registered slash command.

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::commands;
use crate::ui::components::modal::{Modal, ModalResult};

const NAME_COLUMN_WIDTH: usize = 12;

pub struct HelpModal;

impl HelpModal {
    pub fn new() -> Self {
        Self
    }

    fn lines(&self) -> Vec<Line<'static>> {
        commands::registry()
            .iter()
            .map(|cmd| {
                let name = format!("/{:<width$}", cmd.name, width = NAME_COLUMN_WIDTH);
                Line::from(vec![
                    Span::styled(name, Style::default().fg(Color::Cyan)),
                    Span::raw(" "),
                    Span::styled(
                        cmd.description.to_string(),
                        Style::default().add_modifier(Modifier::DIM),
                    ),
                ])
            })
            .collect()
    }
}

impl Modal for HelpModal {
    fn height(&self, _width: u16) -> u16 {
        // top border + N command rows + bottom border. Add 1 for the dismiss hint.
        (commands::registry().len() as u16).saturating_add(3)
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        let border_style = Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM);
        let title = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "help",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]);
        let hint = Line::from(Span::styled(
            " esc / enter to dismiss ",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ));
        let block = Block::default()
            .borders(Borders::TOP | Borders::BOTTOM)
            .border_style(border_style)
            .title(title)
            .title_bottom(hint);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        frame.render_widget(
            Paragraph::new(self.lines()).wrap(Wrap { trim: false }),
            inner,
        );
    }

    fn handle_key(&mut self, key: KeyEvent) -> ModalResult {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => ModalResult::Dismiss,
            _ => ModalResult::Consumed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn esc_dismisses() {
        let mut modal = HelpModal::new();
        assert!(matches!(
            modal.handle_key(key(KeyCode::Esc)),
            ModalResult::Dismiss
        ));
    }

    #[test]
    fn enter_dismisses() {
        let mut modal = HelpModal::new();
        assert!(matches!(
            modal.handle_key(key(KeyCode::Enter)),
            ModalResult::Dismiss
        ));
    }

    #[test]
    fn other_keys_are_consumed_not_forwarded() {
        let mut modal = HelpModal::new();
        // 'x' is not a dismiss key, but it should still be eaten so it
        // doesn't end up typed into the composer behind the modal.
        assert!(matches!(
            modal.handle_key(key(KeyCode::Char('x'))),
            ModalResult::Consumed
        ));
    }

    #[test]
    fn height_matches_registry_size() {
        let modal = HelpModal::new();
        let expected = commands::registry().len() as u16 + 3;
        assert_eq!(modal.height(80), expected);
    }
}
