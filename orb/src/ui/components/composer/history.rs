use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

pub struct PromptHistory {
    pub entries: VecDeque<String>,
    pub path: PathBuf,
    pub max: usize,
}

impl PromptHistory {
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
