use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;

use crate::stats::CellStats;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellPreview {
    pub cell_id: String,
    pub scenario: String,
    pub model: String,
    pub prompt: String,
    pub total_runs: u32,
}

pub trait ProgressReporter: Send + Sync {
    fn suite_started(&self, suite_name: &str, cells: &[CellPreview]);
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
    fn cell_finished(&self, cell_id: &str, stats: &CellStats);
    fn suite_finished(&self);
}

pub struct NoopReporter;

impl ProgressReporter for NoopReporter {
    fn suite_started(&self, _: &str, _: &[CellPreview]) {}
    fn cell_started(&self, _: &str, _: &str, _: &str, _: &str, _: u32) {}
    fn run_started(&self, _: &str, _: u32, _: bool) {}
    fn token_received(&self, _: &str) {}
    fn run_finished(&self, _: &str, _: bool) {}
    fn cell_finished(&self, _: &str, _: &CellStats) {}
    fn suite_finished(&self) {}
}

fn cell_label(scenario: &str, model: &str, prompt: &str) -> String {
    format!("{scenario}·{model}·{prompt}")
}

fn format_elapsed(d: Duration) -> String {
    let secs = d.as_secs();
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h {m}m {s}s")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

struct CellState {
    bar: ProgressBar,
    label: String,
    total_runs: u32,
    run_idx: u32,
    is_warmup: bool,
    chunks: u64,
}

struct ReporterState {
    header: Option<ProgressBar>,
    suite_name: String,
    started: Option<Instant>,
    total_runs_overall: u32,
    done_runs: u32,
    total_cells: usize,
    cells: HashMap<String, CellState>,
}

impl ReporterState {
    fn new() -> Self {
        Self {
            header: None,
            suite_name: String::new(),
            started: None,
            total_runs_overall: 0,
            done_runs: 0,
            total_cells: 0,
            cells: HashMap::new(),
        }
    }

    fn render_header(&self) -> String {
        let elapsed = self
            .started
            .map(|i| format_elapsed(i.elapsed()))
            .unwrap_or_else(|| "0s".into());
        format!(
            "Suite: {} · {} cells · {} runs · {}/{} runs done · {} elapsed",
            self.suite_name,
            self.total_cells,
            self.total_runs_overall,
            self.done_runs,
            self.total_runs_overall,
            elapsed,
        )
    }
}

pub struct IndicatifReporter {
    multi: MultiProgress,
    state: Mutex<ReporterState>,
}

impl IndicatifReporter {
    pub fn new() -> Self {
        Self {
            multi: MultiProgress::new(),
            state: Mutex::new(ReporterState::new()),
        }
    }

    fn header_style() -> ProgressStyle {
        ProgressStyle::with_template("{wide_msg:.bold}").expect("valid template")
    }

    fn pending_style() -> ProgressStyle {
        ProgressStyle::with_template("{prefix} {wide_msg}").expect("valid template")
    }

    fn active_style() -> ProgressStyle {
        ProgressStyle::with_template("{spinner:.cyan} {prefix} {wide_msg}")
            .expect("valid template")
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ")
    }

    fn pending_prefix(label: &str) -> String {
        format!("  {}", label.dimmed())
    }

    fn active_prefix(label: &str) -> String {
        format!("{}", label.bold())
    }

    fn done_prefix(label: &str, success: bool) -> String {
        if success {
            format!("{} {}", "✓".green(), label)
        } else {
            format!("{} {}", "✗".red(), label)
        }
    }

    fn refresh_header(state: &ReporterState) {
        if let Some(h) = state.header.as_ref() {
            h.set_message(state.render_header());
        }
    }

    fn render_active_message(state: &CellState) -> String {
        let phase = if state.is_warmup { "warmup" } else { "run" };
        format!(
            "[{} {}/{}] {} chunks",
            phase,
            state.run_idx + 1,
            state.total_runs,
            state.chunks,
        )
    }

    fn render_done_message(stats: &CellStats) -> String {
        let runs = format!("[{}/{} done]", stats.success_runs, stats.total_runs);
        let mut parts = vec![runs];
        if let Some(ttft) = stats.ttft_ms_p50 {
            parts.push(format!("{:.0}ms TTFT", ttft));
        }
        if let Some(tok_s) = stats.decode_tok_s_mean {
            parts.push(format!("{:.1} tok/s", tok_s));
        }
        parts.join("   ")
    }
}

impl Default for IndicatifReporter {
    fn default() -> Self {
        Self::new()
    }
}

impl ProgressReporter for IndicatifReporter {
    fn suite_started(&self, suite_name: &str, cells: &[CellPreview]) {
        let mut state = self.state.lock().unwrap();
        state.suite_name = suite_name.to_string();
        state.started = Some(Instant::now());
        state.total_cells = cells.len();
        state.total_runs_overall = cells.iter().map(|c| c.total_runs).sum();
        state.done_runs = 0;

        let header = self.multi.add(ProgressBar::new_spinner());
        header.set_style(Self::header_style());
        header.set_message(state.render_header());
        state.header = Some(header);

        let max_label = cells
            .iter()
            .map(|c| cell_label(&c.scenario, &c.model, &c.prompt).chars().count())
            .max()
            .unwrap_or(0);

        for preview in cells {
            let raw = cell_label(&preview.scenario, &preview.model, &preview.prompt);
            let pad = max_label.saturating_sub(raw.chars().count());
            let label = format!("{raw}{}", " ".repeat(pad));
            let bar = self.multi.add(ProgressBar::new_spinner());
            bar.set_style(Self::pending_style());
            bar.set_prefix(Self::pending_prefix(&label));
            bar.set_message(format!("{}", "pending".dimmed()));
            state.cells.insert(
                preview.cell_id.clone(),
                CellState {
                    bar,
                    label,
                    total_runs: preview.total_runs,
                    run_idx: 0,
                    is_warmup: false,
                    chunks: 0,
                },
            );
        }
    }

    fn cell_started(
        &self,
        cell_id: &str,
        _scenario: &str,
        _model: &str,
        _prompt: &str,
        _total_runs: u32,
    ) {
        let state = self.state.lock().unwrap();
        if let Some(cell) = state.cells.get(cell_id) {
            cell.bar.set_style(Self::active_style());
            cell.bar.set_prefix(Self::active_prefix(&cell.label));
            cell.bar.set_message(Self::render_active_message(cell));
            cell.bar.enable_steady_tick(Duration::from_millis(100));
        }
    }

    fn run_started(&self, cell_id: &str, run_idx: u32, is_warmup: bool) {
        let mut state = self.state.lock().unwrap();
        if let Some(cell) = state.cells.get_mut(cell_id) {
            cell.run_idx = run_idx;
            cell.is_warmup = is_warmup;
            cell.chunks = 0;
            cell.bar.set_message(Self::render_active_message(cell));
        }
    }

    fn token_received(&self, cell_id: &str) {
        let mut state = self.state.lock().unwrap();
        if let Some(cell) = state.cells.get_mut(cell_id) {
            cell.chunks += 1;
            cell.bar.set_message(Self::render_active_message(cell));
        }
    }

    fn run_finished(&self, _cell_id: &str, _success: bool) {
        let mut state = self.state.lock().unwrap();
        state.done_runs += 1;
        Self::refresh_header(&state);
    }

    fn cell_finished(&self, cell_id: &str, stats: &CellStats) {
        let state = self.state.lock().unwrap();
        if let Some(cell) = state.cells.get(cell_id) {
            cell.bar.disable_steady_tick();
            cell.bar.set_style(Self::pending_style());
            let success = stats.success_runs == stats.total_runs && stats.total_runs > 0;
            cell.bar.set_prefix(Self::done_prefix(&cell.label, success));
            cell.bar
                .finish_with_message(Self::render_done_message(stats));
        }
    }

    fn suite_finished(&self) {
        let state = self.state.lock().unwrap();
        if let Some(h) = state.header.as_ref() {
            h.set_message(state.render_header());
            h.finish();
        }
    }
}

#[cfg(test)]
pub mod testing {
    use std::sync::Mutex;

    use super::{CellPreview, ProgressReporter};
    use crate::stats::CellStats;

    #[derive(Debug, Clone, PartialEq)]
    pub enum Event {
        SuiteStarted {
            suite_name: String,
            cells: Vec<CellPreview>,
        },
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
            stats: CellStats,
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
        fn suite_started(&self, suite_name: &str, cells: &[CellPreview]) {
            self.events.lock().unwrap().push(Event::SuiteStarted {
                suite_name: suite_name.into(),
                cells: cells.to_vec(),
            });
        }
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
        fn cell_finished(&self, cell_id: &str, stats: &CellStats) {
            self.events.lock().unwrap().push(Event::CellFinished {
                cell_id: cell_id.into(),
                stats: stats.clone(),
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

    fn dummy_stats() -> CellStats {
        CellStats {
            scenario: "s".into(),
            provider: "p".into(),
            model: "m".into(),
            prompt: "pr".into(),
            total_runs: 1,
            success_runs: 1,
            error_runs: 0,
            ttft_ms_p50: Some(10.0),
            ttft_ms_p95: Some(10.0),
            decode_tok_s_mean: Some(20.0),
            decode_tok_s_p50: Some(20.0),
            decode_tok_s_p95: Some(20.0),
            e2e_ms_p50: Some(50.0),
            output_tokens_mean: Some(8.0),
        }
    }

    #[test]
    fn fake_reporter_records_events_in_order() {
        let r = testing::FakeReporter::new();
        let preview = CellPreview {
            cell_id: "c".into(),
            scenario: "s".into(),
            model: "m".into(),
            prompt: "p".into(),
            total_runs: 1,
        };
        r.suite_started("suite", std::slice::from_ref(&preview));
        r.cell_started("c", "s", "m", "p", 1);
        r.run_started("c", 0, false);
        r.token_received("c");
        r.run_finished("c", true);
        r.cell_finished("c", &dummy_stats());
        r.suite_finished();
        let evs = r.events();
        assert_eq!(evs.len(), 7);
        assert!(matches!(
            &evs[0],
            testing::Event::SuiteStarted { suite_name, cells }
                if suite_name == "suite" && cells.len() == 1 && cells[0].cell_id == "c"
        ));
        assert!(matches!(&evs[1], testing::Event::CellStarted { .. }));
        assert!(matches!(
            &evs[5],
            testing::Event::CellFinished { stats, .. } if stats.success_runs == 1
        ));
        assert!(matches!(evs[6], testing::Event::SuiteFinished));
    }
}
