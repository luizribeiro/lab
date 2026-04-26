use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

pub trait ProgressReporter: Send + Sync {
    fn cell_started(
        &self,
        cell_id: &str,
        scenario: &str,
        model: &str,
        prompt: &str,
        total_runs: u32,
    );
    fn run_started(&self, cell_id: &str, run_idx: u32, is_warmup: bool);
    fn token_received(&self, cell_id: &str);
    fn run_finished(&self, cell_id: &str, success: bool);
    fn cell_finished(&self, cell_id: &str);
    fn suite_finished(&self);
}

pub struct NoopReporter;

impl ProgressReporter for NoopReporter {
    fn cell_started(&self, _: &str, _: &str, _: &str, _: &str, _: u32) {}
    fn run_started(&self, _: &str, _: u32, _: bool) {}
    fn token_received(&self, _: &str) {}
    fn run_finished(&self, _: &str, _: bool) {}
    fn cell_finished(&self, _: &str) {}
    fn suite_finished(&self) {}
}

struct CellState {
    bar: ProgressBar,
    scenario: String,
    model: String,
    prompt: String,
    total_runs: u32,
    run_idx: u32,
    is_warmup: bool,
    tokens: u64,
    first_token_at: Option<Instant>,
}

pub struct IndicatifReporter {
    multi: MultiProgress,
    cells: Mutex<HashMap<String, CellState>>,
}

impl IndicatifReporter {
    pub fn new() -> Self {
        Self {
            multi: MultiProgress::new(),
            cells: Mutex::new(HashMap::new()),
        }
    }

    fn style() -> ProgressStyle {
        ProgressStyle::with_template("{spinner:.cyan} {prefix:.bold} {wide_msg}")
            .expect("valid template")
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
    }

    fn update_bar(&self, cell_id: &str, f: impl FnOnce(&mut CellState)) {
        let mut cells = self.cells.lock().unwrap();
        let Some(state) = cells.get_mut(cell_id) else {
            return;
        };
        f(state);
        let msg = Self::render_message(state);
        state.bar.set_message(msg);
    }

    fn render_message(state: &CellState) -> String {
        let phase = if state.is_warmup { "warmup" } else { "run" };
        let mut msg = format!(
            "{}/{}/{} [{} {}/{}] {} tok",
            state.scenario,
            state.model,
            state.prompt,
            phase,
            state.run_idx + 1,
            state.total_runs,
            state.tokens,
        );
        if let Some(first) = state.first_token_at {
            let since_first = Instant::now().saturating_duration_since(first);
            if since_first > Duration::ZERO && state.tokens > 1 {
                let rate = (state.tokens - 1) as f64 / since_first.as_secs_f64();
                msg.push_str(&format!(" • {rate:.1} tok/s"));
            }
        }
        msg
    }
}

impl Default for IndicatifReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressReporter for IndicatifReporter {
    fn cell_started(
        &self,
        cell_id: &str,
        scenario: &str,
        model: &str,
        prompt: &str,
        total_runs: u32,
    ) {
        let bar = self.multi.add(ProgressBar::new_spinner());
        bar.set_style(Self::style());
        bar.set_prefix(cell_id.to_string());
        bar.enable_steady_tick(Duration::from_millis(100));
        let state = CellState {
            bar,
            scenario: scenario.to_string(),
            model: model.to_string(),
            prompt: prompt.to_string(),
            total_runs,
            run_idx: 0,
            is_warmup: false,
            tokens: 0,
            first_token_at: None,
        };
        let msg = Self::render_message(&state);
        state.bar.set_message(msg);
        self.cells
            .lock()
            .unwrap()
            .insert(cell_id.to_string(), state);
    }

    fn run_started(&self, cell_id: &str, run_idx: u32, is_warmup: bool) {
        self.update_bar(cell_id, |state| {
            state.run_idx = run_idx;
            state.is_warmup = is_warmup;
            state.tokens = 0;
            state.first_token_at = None;
        });
    }

    fn token_received(&self, cell_id: &str) {
        self.update_bar(cell_id, |state| {
            state.tokens += 1;
            if state.first_token_at.is_none() {
                state.first_token_at = Some(Instant::now());
            }
        });
    }

    fn run_finished(&self, _cell_id: &str, _success: bool) {}

    fn cell_finished(&self, cell_id: &str) {
        let mut cells = self.cells.lock().unwrap();
        if let Some(state) = cells.remove(cell_id) {
            state.bar.finish_and_clear();
        }
    }

    fn suite_finished(&self) {
        let _ = self.multi.clear();
    }
}

#[cfg(test)]
pub mod testing {
    use std::sync::Mutex;

    use super::ProgressReporter;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Event {
        CellStarted {
            cell_id: String,
            scenario: String,
            model: String,
            prompt: String,
            total_runs: u32,
        },
        RunStarted {
            cell_id: String,
            run_idx: u32,
            is_warmup: bool,
        },
        TokenReceived {
            cell_id: String,
        },
        RunFinished {
            cell_id: String,
            success: bool,
        },
        CellFinished {
            cell_id: String,
        },
        SuiteFinished,
    }

    #[derive(Default)]
    pub struct FakeReporter {
        events: Mutex<Vec<Event>>,
    }

    impl FakeReporter {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn events(&self) -> Vec<Event> {
            self.events.lock().unwrap().clone()
        }
    }

    impl ProgressReporter for FakeReporter {
        fn cell_started(
            &self,
            cell_id: &str,
            scenario: &str,
            model: &str,
            prompt: &str,
            total_runs: u32,
        ) {
            self.events.lock().unwrap().push(Event::CellStarted {
                cell_id: cell_id.into(),
                scenario: scenario.into(),
                model: model.into(),
                prompt: prompt.into(),
                total_runs,
            });
        }
        fn run_started(&self, cell_id: &str, run_idx: u32, is_warmup: bool) {
            self.events.lock().unwrap().push(Event::RunStarted {
                cell_id: cell_id.into(),
                run_idx,
                is_warmup,
            });
        }
        fn token_received(&self, cell_id: &str) {
            self.events.lock().unwrap().push(Event::TokenReceived {
                cell_id: cell_id.into(),
            });
        }
        fn run_finished(&self, cell_id: &str, success: bool) {
            self.events.lock().unwrap().push(Event::RunFinished {
                cell_id: cell_id.into(),
                success,
            });
        }
        fn cell_finished(&self, cell_id: &str) {
            self.events.lock().unwrap().push(Event::CellFinished {
                cell_id: cell_id.into(),
            });
        }
        fn suite_finished(&self) {
            self.events.lock().unwrap().push(Event::SuiteFinished);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_reporter_records_events_in_order() {
        let r = testing::FakeReporter::new();
        r.cell_started("c", "s", "m", "p", 1);
        r.run_started("c", 0, false);
        r.token_received("c");
        r.run_finished("c", true);
        r.cell_finished("c");
        r.suite_finished();
        let evs = r.events();
        assert_eq!(evs.len(), 6);
        assert!(matches!(evs[0], testing::Event::CellStarted { .. }));
        assert!(matches!(evs[5], testing::Event::SuiteFinished));
    }
}
