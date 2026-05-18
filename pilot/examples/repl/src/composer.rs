use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tui_textarea::TextArea;

use crate::app::{Term, VIEWPORT_HEIGHT};

pub struct Composer {
    pub textarea: TextArea<'static>,
    pub history: History,
    pub search: Option<Search>,
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
        let mut textarea = TextArea::default();
        textarea.set_cursor_line_style(Default::default());
        Self {
            textarea,
            history,
            search: None,
        }
    }

    pub fn input(&mut self, key: KeyEvent) {
        self.textarea.input(key);
    }

    pub fn take_input(&mut self) -> String {
        let lines = self.textarea.lines();
        let text = lines.join("\n").trim().to_string();
        self.textarea = TextArea::default();
        text
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
                    self.textarea = TextArea::new(entry.lines().map(String::from).collect());
                }
                self.search = None;
            }
            (KeyCode::Char('r'), m) if m.contains(KeyModifiers::CONTROL) => {
                let before = search
                    .match_idx
                    .unwrap_or(self.history.entries.len())
                    .saturating_sub(1);
                if let Some(new_idx) =
                    find_match(&self.history.entries, &search.query, before + 1)
                {
                    search.match_idx = Some(new_idx);
                }
            }
            (KeyCode::Backspace, _) => {
                search.query.pop();
                search.match_idx =
                    find_match(&self.history.entries, &search.query, self.history.entries.len());
            }
            (KeyCode::Char(c), _) => {
                search.query.push(c);
                search.match_idx =
                    find_match(&self.history.entries, &search.query, self.history.entries.len());
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
        // Re-anchor the inline viewport at the current cursor position. The
        // simplest reliable way to do this in ratatui 0.30 is to drop and
        // recreate the Terminal.
        let backend = CrosstermBackend::new(io::stdout());
        *terminal = Terminal::with_options(
            backend,
            ratatui::TerminalOptions {
                viewport: ratatui::Viewport::Inline(VIEWPORT_HEIGHT),
            },
        )?;

        if let Ok(new) = result {
            self.textarea = TextArea::new(new.lines().map(String::from).collect());
        }
        Ok(())
    }
}

impl History {
    pub fn load(path: PathBuf) -> Self {
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
        .prefix("pilot-prompt-")
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
