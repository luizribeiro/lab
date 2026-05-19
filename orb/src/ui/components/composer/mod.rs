use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Modifier, Style};
use tui_textarea::TextArea;

use crate::ui::terminal::{Term, make_terminal};

pub struct Composer {
    pub textarea: TextArea<'static>,
    pub history: History,
    pub search: Option<Search>,
    history_cursor: Option<usize>,
    history_draft: Option<Vec<String>>,
    focused: bool,
}

pub struct History {
    pub entries: VecDeque<String>,
    pub path: PathBuf,
    pub max: usize,
}

pub struct Search {
    pub query: String,
    pub match_idx: Option<usize>,
}

impl Composer {
    pub fn new(history_path: PathBuf) -> Self {
        let history = History::load(history_path);
        Self {
            textarea: new_textarea(Vec::new(), true),
            history,
            search: None,
            history_cursor: None,
            history_draft: None,
            focused: true,
        }
    }

    pub fn input(&mut self, key: KeyEvent) {
        self.reset_history_navigation();
        self.textarea.input(key);
    }

    pub fn take_input(&mut self) -> String {
        let lines = self.textarea.lines();
        let text = lines.join("\n").trim().to_string();
        self.textarea = new_textarea(Vec::new(), self.focused);
        self.reset_history_navigation();
        text
    }

    pub fn set_focused(&mut self, focused: bool) {
        if self.focused == focused {
            return;
        }
        self.focused = focused;
        self.textarea.set_cursor_style(cursor_style(focused));
    }

    pub fn history_previous(&mut self) {
        if self.history.entries.is_empty() {
            return;
        }

        let idx = match self.history_cursor {
            Some(idx) => idx.saturating_sub(1),
            None => {
                self.history_draft = Some(self.textarea.lines().to_vec());
                self.history.entries.len() - 1
            }
        };
        self.history_cursor = Some(idx);
        self.set_text(self.history.entries[idx].clone());
    }

    pub fn history_next(&mut self) {
        let Some(cursor) = self.history_cursor else {
            return;
        };

        if cursor + 1 < self.history.entries.len() {
            let idx = cursor + 1;
            self.history_cursor = Some(idx);
            self.set_text(self.history.entries[idx].clone());
        } else {
            let draft = self.history_draft.take().unwrap_or_default();
            self.history_cursor = None;
            self.textarea = new_textarea(draft, self.focused);
        }
    }

    pub fn start_search(&mut self) {
        let initial = find_match(&self.history.entries, "", self.history.entries.len());
        self.search = Some(Search {
            query: String::new(),
            match_idx: initial,
        });
    }

    pub fn is_searching(&self) -> bool {
        self.search.is_some()
    }

    pub fn handle_search_key(&mut self, key: KeyEvent) {
        let Some(search) = self.search.as_mut() else {
            return;
        };
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                self.search = None;
            }
            (KeyCode::Char('c' | 'g'), m) if m.contains(KeyModifiers::CONTROL) => {
                self.search = None;
            }
            (KeyCode::Enter, _) => {
                if let Some(idx) = search.match_idx {
                    let entry = self.history.entries[idx].clone();
                    self.set_text(entry);
                }
                self.search = None;
                self.reset_history_navigation();
            }
            (KeyCode::Char('r'), m) if m.contains(KeyModifiers::CONTROL) => {
                let before = search
                    .match_idx
                    .unwrap_or(self.history.entries.len())
                    .saturating_sub(1);
                if let Some(new_idx) = find_match(&self.history.entries, &search.query, before + 1)
                {
                    search.match_idx = Some(new_idx);
                }
            }
            (KeyCode::Backspace, _) => {
                search.query.pop();
                search.match_idx = find_match(
                    &self.history.entries,
                    &search.query,
                    self.history.entries.len(),
                );
            }
            (KeyCode::Char(c), _) => {
                search.query.push(c);
                search.match_idx = find_match(
                    &self.history.entries,
                    &search.query,
                    self.history.entries.len(),
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
        // Re-anchor the inline viewport at the current cursor position.
        // Dropping and recreating the Terminal is the simplest way; the
        // exact viewport height doesn't matter here because the main loop
        // will resize it on the next iteration to match the actual
        // composer state.
        *terminal = make_terminal(3)?;

        if let Ok(new) = result {
            self.set_text(new);
            self.reset_history_navigation();
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
    /// flows (e.g. slash-command autocomplete). Resets history navigation
    /// since the user clearly didn't pick that history entry.
    pub fn replace_text(&mut self, text: String) {
        self.set_text(text);
        for _ in 0..self.textarea.lines().len().saturating_sub(1) {
            self.textarea
                .input(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        self.textarea
            .input(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        self.reset_history_navigation();
    }

    fn reset_history_navigation(&mut self) {
        self.history_cursor = None;
        self.history_draft = None;
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

impl History {
    pub fn load(path: PathBuf) -> Self {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let entries = std::fs::read_to_string(&path)
            .map(|content| content.lines().map(String::from).collect::<VecDeque<_>>())
            .unwrap_or_default();
        Self {
            entries,
            path,
            max: 2000,
        }
    }

    pub fn push(&mut self, entry: String) {
        if entry.is_empty() {
            return;
        }
        if self.entries.back().is_some_and(|s| s == &entry) {
            return;
        }
        self.entries.push_back(entry.clone());
        while self.entries.len() > self.max {
            self.entries.pop_front();
        }
        let _ = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .and_then(|mut f| writeln!(f, "{}", entry.replace('\n', " ")));
    }
}

/// Find the most recent (highest-index) history entry strictly before
/// `before` that contains `query`. Empty query matches the most recent entry.
fn find_match(entries: &VecDeque<String>, query: &str, before: usize) -> Option<usize> {
    let upper = before.min(entries.len());
    if upper == 0 {
        return None;
    }
    if query.is_empty() {
        return Some(upper - 1);
    }
    (0..upper).rev().find(|&i| entries[i].contains(query))
}

fn run_editor(initial: &str) -> io::Result<String> {
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());
    let mut tmp = tempfile::Builder::new()
        .prefix("orb-prompt-")
        .suffix(".md")
        .tempfile()?;
    tmp.write_all(initial.as_bytes())?;
    tmp.flush()?;
    let (file, path) = tmp.keep().map_err(io::Error::other)?;
    drop(file);
    let status = std::process::Command::new(&editor).arg(&path).status()?;
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    if !status.success() {
        return Err(io::Error::other("editor exited non-zero"));
    }
    Ok(content.trim_end_matches('\n').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn composer_with_history(entries: &[&str]) -> Composer {
        let mut composer = Composer::new(PathBuf::from("/tmp/orb-test-history"));
        composer.history.entries = entries.iter().map(|entry| entry.to_string()).collect();
        composer
    }

    fn text(composer: &Composer) -> String {
        composer.textarea.lines().join("\n")
    }

    #[test]
    fn up_and_down_walk_sent_message_history() {
        let mut composer = composer_with_history(&["one", "two", "three"]);

        composer.history_previous();
        assert_eq!(text(&composer), "three");

        composer.history_previous();
        assert_eq!(text(&composer), "two");

        composer.history_next();
        assert_eq!(text(&composer), "three");
    }

    #[test]
    fn down_after_newest_restores_current_draft() {
        let mut composer = composer_with_history(&["one", "two"]);
        composer.input(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE));
        composer.input(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        composer.input(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        composer.input(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE));
        composer.input(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));

        composer.history_previous();
        assert_eq!(text(&composer), "two");

        composer.history_next();
        assert_eq!(text(&composer), "draft");
    }
}
