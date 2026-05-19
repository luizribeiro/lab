//! Slash-command autocomplete modal.
//!
//! Pushed when the user starts typing a `/` at the composer. Updates its
//! match list as the composer text changes, navigates with Up/Down, completes
//! into the composer on Tab/Enter, and dismisses itself on Esc or when the
//! user backspaces away the leading `/`.
//!
//! Letter keys are Forwarded back to the composer so typing flows through
//! the modal — this is the test case for [`ModalResult::Forward`].

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::commands::{self, Command};
use crate::modal::{Modal, ModalEffect, ModalResult};

const MAX_VISIBLE_ROWS: usize = 6;
const NAME_COLUMN_WIDTH: usize = 12;

pub struct SlashAutocompleteModal {
    /// What the user has typed so far, with the leading `/` (e.g. `/he`).
    filter: String,
    /// Index into the current `matches()` list. Clamped on render.
    selected: usize,
    /// Filled in by the Tab/Enter path; read by `take_effect`.
    pending_effect: Option<String>,
}

impl SlashAutocompleteModal {
    pub fn new(initial_filter: &str) -> Self {
        Self {
            filter: initial_filter.to_string(),
            selected: 0,
            pending_effect: None,
        }
    }

    fn matches(&self) -> Vec<&'static Command> {
        let needle = self.filter.strip_prefix('/').unwrap_or(&self.filter);
        commands::registry()
            .iter()
            .copied()
            .filter(|c| c.name.starts_with(needle))
            .collect()
    }

    fn visible_count(&self) -> usize {
        self.matches().len().min(MAX_VISIBLE_ROWS).max(1)
    }

    fn clamped_selected(&self) -> usize {
        let n = self.matches().len();
        if n == 0 {
            0
        } else {
            self.selected.min(n - 1)
        }
    }

    fn lines(&self) -> Vec<Line<'static>> {
        let matches = self.matches();
        if matches.is_empty() {
            return vec![Line::from(Span::styled(
                " no matches".to_string(),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::DIM),
            ))];
        }

        let selected = self.clamped_selected();
        let visible_end = matches.len().min(MAX_VISIBLE_ROWS);
        matches
            .iter()
            .take(visible_end)
            .enumerate()
            .map(|(i, cmd)| {
                let is_selected = i == selected;
                let name = format!("/{:<width$}", cmd.name, width = NAME_COLUMN_WIDTH);
                let row_style = if is_selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().fg(Color::Cyan)
                };
                let desc_style = if is_selected {
                    Style::default().fg(Color::Black).bg(Color::Cyan)
                } else {
                    Style::default().add_modifier(Modifier::DIM)
                };
                Line::from(vec![
                    Span::styled(name, row_style),
                    Span::raw(" "),
                    Span::styled(cmd.description.to_string(), desc_style),
                ])
            })
            .collect()
    }
}

impl Modal for SlashAutocompleteModal {
    fn height(&self, _width: u16) -> u16 {
        // top border + visible rows + bottom border.
        (self.visible_count() as u16).saturating_add(2)
    }

    fn render(&mut self, area: Rect, frame: &mut Frame) {
        let border_style = Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::DIM);
        let title = Line::from(vec![
            Span::raw(" "),
            Span::styled(
                "commands",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
        ]);
        let hint = Line::from(Span::styled(
            " ↑/↓ select · tab/enter complete · esc cancel ",
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
        frame.render_widget(Paragraph::new(self.lines()), inner);
    }

    fn handle_key(&mut self, key: KeyEvent) -> ModalResult {
        match key.code {
            KeyCode::Up => {
                self.selected = self.clamped_selected().saturating_sub(1);
                ModalResult::Consumed
            }
            KeyCode::Down => {
                let n = self.matches().len();
                if n > 0 {
                    self.selected = (self.clamped_selected() + 1).min(n - 1);
                }
                ModalResult::Consumed
            }
            KeyCode::Tab | KeyCode::Enter => {
                let matches = self.matches();
                if let Some(cmd) = matches.get(self.clamped_selected()) {
                    self.pending_effect = Some(format!("/{}", cmd.name));
                }
                ModalResult::Dismiss
            }
            KeyCode::Esc => ModalResult::Dismiss,
            _ => ModalResult::Forward,
        }
    }

    fn on_composer_change(&mut self, text: &str) -> bool {
        // User backspaced past the `/` — autocomplete no longer applies.
        if !text.starts_with('/') {
            return true;
        }
        self.filter = text.to_string();
        // Reset selection so it doesn't point past the new (likely shorter) match list.
        self.selected = 0;
        false
    }

    fn take_effect(&mut self) -> Option<ModalEffect> {
        self.pending_effect.take().map(ModalEffect::ReplaceComposer)
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
    fn empty_filter_matches_all() {
        let modal = SlashAutocompleteModal::new("/");
        assert_eq!(modal.matches().len(), commands::registry().len());
    }

    #[test]
    fn prefix_filters_matches() {
        let modal = SlashAutocompleteModal::new("/he");
        let matches = modal.matches();
        assert!(matches.iter().any(|c| c.name == "help"));
        assert!(matches.iter().all(|c| c.name.starts_with("he")));
    }

    #[test]
    fn enter_emits_replacement_for_selected_command() {
        let mut modal = SlashAutocompleteModal::new("/exi");
        assert!(matches!(modal.handle_key(key(KeyCode::Enter)), ModalResult::Dismiss));
        assert_eq!(modal.take_effect().and_then(|e| match e {
            ModalEffect::ReplaceComposer(s) => Some(s),
        }), Some("/exit".to_string()));
    }

    #[test]
    fn esc_dismisses_without_effect() {
        let mut modal = SlashAutocompleteModal::new("/exi");
        assert!(matches!(modal.handle_key(key(KeyCode::Esc)), ModalResult::Dismiss));
        assert!(modal.take_effect().is_none());
    }

    #[test]
    fn down_advances_selection_then_clamps() {
        let mut modal = SlashAutocompleteModal::new("/");
        let total = modal.matches().len();
        for _ in 0..(total + 5) {
            modal.handle_key(key(KeyCode::Down));
        }
        assert_eq!(modal.clamped_selected(), total - 1);
    }

    #[test]
    fn non_navigation_keys_forward_to_composer() {
        let mut modal = SlashAutocompleteModal::new("/");
        assert!(matches!(
            modal.handle_key(key(KeyCode::Char('h'))),
            ModalResult::Forward
        ));
        assert!(matches!(
            modal.handle_key(key(KeyCode::Backspace)),
            ModalResult::Forward
        ));
    }

    #[test]
    fn on_composer_change_dismisses_when_slash_removed() {
        let mut modal = SlashAutocompleteModal::new("/he");
        assert!(modal.on_composer_change("hello"));
    }

    #[test]
    fn on_composer_change_updates_filter() {
        let mut modal = SlashAutocompleteModal::new("/");
        assert!(!modal.on_composer_change("/red"));
        assert!(modal.matches().iter().any(|c| c.name == "redraw"));
    }
}
