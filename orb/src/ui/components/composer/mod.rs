use std::io;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Modifier, Style};
use tui_textarea::TextArea;

use crate::ui::terminal::{Term, make_terminal};

mod editor;
mod history;
mod search;

use editor::run_editor;
pub use history::PromptHistory;
pub use search::ReversePromptSearch;
use search::find_match;

pub struct Composer {
    pub textarea: TextArea<'static>,
    pub prompt_history: PromptHistory,
    pub reverse_prompt_search: Option<ReversePromptSearch>,
    prompt_history_cursor: Option<usize>,
    draft_before_history_navigation: Option<Vec<String>>,
    focused: bool,
}

impl Composer {
    pub fn new(prompt_history_path: PathBuf) -> Self {
        let prompt_history = PromptHistory::load(prompt_history_path);
        Self {
            textarea: new_textarea(Vec::new(), true),
            prompt_history,
            reverse_prompt_search: None,
            prompt_history_cursor: None,
            draft_before_history_navigation: None,
            focused: true,
        }
    }

    pub fn input(&mut self, key: KeyEvent) {
        self.reset_prompt_history_navigation();
        self.textarea.input(key);
    }

    pub fn take_input(&mut self) -> String {
        let lines = self.textarea.lines();
        let text = lines.join("\n").trim().to_string();
        self.textarea = new_textarea(Vec::new(), self.focused);
        self.reset_prompt_history_navigation();
        text
    }

    pub fn set_focused(&mut self, focused: bool) {
        if self.focused == focused {
            return;
        }
        self.focused = focused;
        self.textarea.set_cursor_style(cursor_style(focused));
    }

    pub fn prompt_history_previous(&mut self) {
        if self.prompt_history.entries.is_empty() {
            return;
        }

        let idx = match self.prompt_history_cursor {
            Some(idx) => idx.saturating_sub(1),
            None => {
                self.draft_before_history_navigation = Some(self.textarea.lines().to_vec());
                self.prompt_history.entries.len() - 1
            }
        };
        self.prompt_history_cursor = Some(idx);
        self.set_text(self.prompt_history.entries[idx].clone());
    }

    pub fn prompt_history_next(&mut self) {
        let Some(cursor) = self.prompt_history_cursor else {
            return;
        };

        if cursor + 1 < self.prompt_history.entries.len() {
            let idx = cursor + 1;
            self.prompt_history_cursor = Some(idx);
            self.set_text(self.prompt_history.entries[idx].clone());
        } else {
            let draft = self
                .draft_before_history_navigation
                .take()
                .unwrap_or_default();
            self.prompt_history_cursor = None;
            self.textarea = new_textarea(draft, self.focused);
        }
    }

    pub fn start_search(&mut self) {
        let initial = find_match(
            &self.prompt_history.entries,
            "",
            self.prompt_history.entries.len(),
        );
        self.reverse_prompt_search = Some(ReversePromptSearch {
            query: String::new(),
            match_idx: initial,
        });
    }

    pub fn is_searching(&self) -> bool {
        self.reverse_prompt_search.is_some()
    }

    pub fn handle_search_key(&mut self, key: KeyEvent) {
        let Some(search) = self.reverse_prompt_search.as_mut() else {
            return;
        };
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                self.reverse_prompt_search = None;
            }
            (KeyCode::Char('c' | 'g'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.reverse_prompt_search = None;
            }
            (KeyCode::Enter, _) => {
                if let Some(idx) = search.match_idx {
                    let entry = self.prompt_history.entries[idx].clone();
                    self.set_text(entry);
                }
                self.reverse_prompt_search = None;
                self.reset_prompt_history_navigation();
            }
            (KeyCode::Char('r'), m) if m.contains(KeyModifiers::CONTROL) => {
                let before = search
                    .match_idx
                    .unwrap_or(self.prompt_history.entries.len())
                    .saturating_sub(1);
                if let Some(new_idx) =
                    find_match(&self.prompt_history.entries, &search.query, before + 1)
                {
                    search.match_idx = Some(new_idx);
                }
            }
            (KeyCode::Backspace, _) => {
                search.query.pop();
                search.match_idx = find_match(
                    &self.prompt_history.entries,
                    &search.query,
                    self.prompt_history.entries.len(),
                );
            }
            (KeyCode::Char(c), _) => {
                search.query.push(c);
                search.match_idx = find_match(
                    &self.prompt_history.entries,
                    &search.query,
                    self.prompt_history.entries.len(),
                );
            }
            _ => {}
        }
    }

    pub async fn open_external_editor(&mut self, terminal: &mut Term) -> io::Result<()> {
        let initial = self.textarea.lines().join("\n");

        let _ = terminal.clear();
        crossterm::terminal::disable_raw_mode()?;

        let result = run_editor(&initial);

        crossterm::terminal::enable_raw_mode()?;
        // Re-anchor the live viewport at the current cursor position.
        // Dropping and recreating the Terminal is the simplest way; the
        // exact viewport height doesn't matter here because the main loop
        // will resize it on the next iteration to match the actual
        // composer state.
        *terminal = make_terminal(3)?;

        if let Ok(new) = result {
            self.set_text(new);
            self.reset_prompt_history_navigation();
        }
        Ok(())
    }

    pub fn set_text(&mut self, text: String) {
        let lines: Vec<String> = if text.is_empty() {
            Vec::new()
        } else {
            text.lines().map(String::from).collect()
        };
        self.textarea = new_textarea(lines, self.focused);
    }

    /// Replace the textarea contents and place the textarea cursor at the
    /// very end of the inserted text. Intended for external "complete-into"
    /// flows (e.g. slash-command autocomplete). Resets prompt history navigation
    /// since the user clearly didn't pick that history entry.
    pub fn replace_text(&mut self, text: String) {
        self.set_text(text);
        for _ in 0..self.textarea.lines().len().saturating_sub(1) {
            self.textarea
                .input(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        self.textarea
            .input(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        self.reset_prompt_history_navigation();
    }

    fn reset_prompt_history_navigation(&mut self) {
        self.prompt_history_cursor = None;
        self.draft_before_history_navigation = None;
    }
}

/// Construct a fresh textarea with our preferred display settings — most
/// importantly, no underline on the cursor line (tui-textarea-2 defaults
/// to `Modifier::UNDERLINED`, which we don't want for a chat prompt).
fn new_textarea(lines: Vec<String>, focused: bool) -> TextArea<'static> {
    let mut textarea = if lines.is_empty() {
        TextArea::default()
    } else {
        TextArea::new(lines)
    };
    textarea.set_cursor_line_style(Style::default());
    textarea.set_cursor_style(cursor_style(focused));
    textarea
}

fn cursor_style(focused: bool) -> Style {
    if focused {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn composer_with_prompt_history(entries: &[&str]) -> Composer {
        let mut composer = Composer::new(PathBuf::from("/tmp/orb-test-history"));
        composer.prompt_history.entries = entries.iter().map(|entry| entry.to_string()).collect();
        composer
    }

    fn text(composer: &Composer) -> String {
        composer.textarea.lines().join("\n")
    }

    #[test]
    fn up_and_down_walk_prompt_history() {
        let mut composer = composer_with_prompt_history(&["one", "two", "three"]);

        composer.prompt_history_previous();
        assert_eq!(text(&composer), "three");

        composer.prompt_history_previous();
        assert_eq!(text(&composer), "two");

        composer.prompt_history_next();
        assert_eq!(text(&composer), "three");
    }

    #[test]
    fn down_after_newest_restores_current_draft() {
        let mut composer = composer_with_prompt_history(&["one", "two"]);
        composer.input(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        composer.input(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        composer.input(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        composer.input(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
        composer.input(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));

        composer.prompt_history_previous();
        assert_eq!(text(&composer), "two");

        composer.prompt_history_next();
        assert_eq!(text(&composer), "draft");
    }
}
