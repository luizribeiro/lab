use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;

use crate::dimensions::Dimensions;
use crate::stats::CellStats;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellPreview {
    pub cell_id: String,
    pub dimensions: Dimensions,
    pub total_runs: u32,
}

pub trait ProgressReporter: Send + Sync {
    fn suite_started(&self, suite_name: &str, cells: &[CellPreview]);
    fn cell_started(&self, cell_id: &str, total_runs: u32);
    fn run_started(&self, cell_id: &str, run_idx: u32, is_warmup: bool);
    fn token_received(&self, cell_id: &str);
    fn run_finished(&self, cell_id: &str, success: bool);
    fn cell_finished(&self, cell_id: &str, stats: &CellStats);
    fn suite_finished(&self);
}

pub struct NoopReporter;

impl ProgressReporter for NoopReporter {
    fn suite_started(&self, _: &str, _: &[CellPreview]) {}
    fn cell_started(&self, _: &str, _: u32) {}
    fn run_started(&self, _: &str, _: u32, _: bool) {}
    fn token_received(&self, _: &str) {}
    fn run_finished(&self, _: &str, _: bool) {}
    fn cell_finished(&self, _: &str, _: &CellStats) {}
    fn suite_finished(&self) {}
}

fn dim_label(dimensions: &Dimensions, varying: &[String]) -> String {
    varying
        .iter()
        .map(|axis| dimensions.axis_value(axis))
        .collect::<Vec<_>>()
        .join(" · ")
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
    total_runs_overall: u32,
    done_runs: u32,
    total_cells: usize,
    varying: Vec<String>,
    cells: HashMap<String, CellState>,
}

impl ReporterState {
    fn new() -> Self {
        Self {
            header: None,
            suite_name: String::new(),
            total_runs_overall: 0,
            done_runs: 0,
            total_cells: 0,
            varying: Vec::new(),
            cells: HashMap::new(),
        }
    }

    fn render_header(&self) -> String {
        let base = format!(
            "Suite: {} · {} cells · {} runs",
            self.suite_name, self.total_cells, self.total_runs_overall,
        );
        if self.varying.is_empty() {
            base
        } else {
            format!("{} · varying: {}", base, self.varying.join(", "))
        }
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

    #[cfg(test)]
    pub fn done_runs(&self) -> u32 {
        self.state.lock().unwrap().done_runs
    }

    fn header_style() -> ProgressStyle {
        ProgressStyle::with_template("{msg:.bold} · {elapsed}").expect("valid template")
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
        if label.is_empty() {
            String::new()
        } else {
            format!("  {}", label.dimmed())
        }
    }

    fn active_prefix(label: &str) -> String {
        if label.is_empty() {
            String::new()
        } else {
            format!("{}", label.bold())
        }
    }

    fn done_prefix(label: &str, success: bool) -> String {
        let mark = if success {
            format!("{}", "✓".green())
        } else {
            format!("{}", "✗".red())
        };
        if label.is_empty() {
            mark
        } else {
            format!("{mark} {label}")
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
        state.total_cells = cells.len();
        state.total_runs_overall = cells.iter().map(|c| c.total_runs).sum();
        state.done_runs = 0;

        let dims: Vec<Dimensions> = cells.iter().map(|c| c.dimensions.clone()).collect();
        state.varying = Dimensions::varying(&dims)
            .into_iter()
            .map(String::from)
            .collect();

        let header = self.multi.add(ProgressBar::new_spinner());
        header.set_style(Self::header_style());
        header.set_message(state.render_header());
        header.enable_steady_tick(Duration::from_secs(1));
        state.header = Some(header);

        let varying = state.varying.clone();
        let max_label = cells
            .iter()
            .map(|c| dim_label(&c.dimensions, &varying).chars().count())
            .max()
            .unwrap_or(0);

        for preview in cells {
            let raw = dim_label(&preview.dimensions, &varying);
            let label = if max_label == 0 {
                String::new()
            } else {
                let pad = max_label.saturating_sub(raw.chars().count());
                format!("{raw}{}", " ".repeat(pad))
            };
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

    fn cell_started(&self, cell_id: &str, _total_runs: u32) {
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

    fn run_finished(&self, cell_id: &str, _success: bool) {
        let mut state = self.state.lock().unwrap();
        let was_warmup = state
            .cells
            .get(cell_id)
            .map(|c| c.is_warmup)
            .unwrap_or(false);
        if !was_warmup {
            state.done_runs += 1;
        }
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
    #[allow(clippy::large_enum_variant)]
    pub enum Event {
        SuiteStarted {
            suite_name: String,
            cells: Vec<CellPreview>,
        },
        CellStarted {
            cell_id: String,
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
        fn cell_started(&self, cell_id: &str, total_runs: u32) {
            self.events.lock().unwrap().push(Event::CellStarted {
                cell_id: cell_id.into(),
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
    use crate::dimensions::test_dimensions;

    fn dummy_stats() -> CellStats {
        CellStats {
            dimensions: test_dimensions("s", "p", "m", "pr"),
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

    fn preview(cell_id: &str, model: &str, prompt: &str, total_runs: u32) -> CellPreview {
        CellPreview {
            cell_id: cell_id.into(),
            dimensions: test_dimensions("s", "p", model, prompt),
            total_runs,
        }
    }

    #[test]
    fn fake_reporter_records_events_in_order() {
        let r = testing::FakeReporter::new();
        let p = preview("c", "m", "pr", 1);
        r.suite_started("suite", std::slice::from_ref(&p));
        r.cell_started("c", 1);
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

    #[test]
    fn indicatif_reporter_excludes_warmup_from_suite_progress() {
        let r = IndicatifReporter::new();
        let p = preview("c", "m", "pr", 2);
        r.suite_started("suite", std::slice::from_ref(&p));
        r.cell_started("c", 2);

        for warmup_idx in 0..3 {
            r.run_started("c", warmup_idx, true);
            r.run_finished("c", true);
        }
        assert_eq!(r.done_runs(), 0, "warmups must not advance suite progress");

        for run_idx in 0..2 {
            r.run_started("c", run_idx, false);
            r.run_finished("c", true);
        }
        assert_eq!(r.done_runs(), 2, "only measured runs count");

        r.cell_finished("c", &dummy_stats());
        r.suite_finished();
    }

    #[test]
    fn header_includes_varying_axes_when_present() {
        let r = IndicatifReporter::new();
        let cells = vec![preview("a", "m1", "pr", 1), preview("b", "m2", "pr", 1)];
        r.suite_started("decode", &cells);
        let header = r.state.lock().unwrap().render_header();
        assert!(header.contains("varying: model"), "got {header:?}");
        assert!(!header.contains("prompt"), "constant axes must not appear");
        assert!(header.contains("decode"));
        assert!(header.contains("2 cells"));
    }

    #[test]
    fn header_omits_varying_when_single_cell() {
        let r = IndicatifReporter::new();
        let p = preview("c", "m", "pr", 1);
        r.suite_started("decode", std::slice::from_ref(&p));
        let header = r.state.lock().unwrap().render_header();
        assert!(!header.contains("varying"), "got {header:?}");
    }

    #[test]
    fn cell_label_is_only_varying_axes_joined() {
        let r = IndicatifReporter::new();
        let cells = vec![preview("a", "m1", "pr", 1), preview("b", "m2", "pr", 1)];
        r.suite_started("suite", &cells);
        let state = r.state.lock().unwrap();
        let a_label = state.cells.get("a").unwrap().label.trim_end().to_string();
        let b_label = state.cells.get("b").unwrap().label.trim_end().to_string();
        assert_eq!(a_label, "m1");
        assert_eq!(b_label, "m2");
        assert_eq!(
            state.cells.get("a").unwrap().label.chars().count(),
            state.cells.get("b").unwrap().label.chars().count(),
            "labels must be padded to equal width",
        );
    }

    #[test]
    fn cell_label_joins_multiple_varying_axes() {
        use crate::var::VarValue;
        use indexmap::IndexMap;

        fn dim(model: &str, max_tokens: i64) -> Dimensions {
            let mut vars: IndexMap<String, VarValue> = IndexMap::new();
            vars.insert("model".into(), VarValue::from(model));
            vars.insert("prompt".into(), VarValue::from("pr"));
            vars.insert("max_tokens".into(), VarValue::from(max_tokens));
            Dimensions {
                scenario: "s".into(),
                provider: "p".into(),
                vars,
            }
        }

        let r = IndicatifReporter::new();
        let cells = vec![
            CellPreview {
                cell_id: "a".into(),
                dimensions: dim("m1", 2048),
                total_runs: 1,
            },
            CellPreview {
                cell_id: "b".into(),
                dimensions: dim("m2", 4096),
                total_runs: 1,
            },
        ];
        r.suite_started("suite", &cells);
        let state = r.state.lock().unwrap();
        assert_eq!(state.cells.get("a").unwrap().label.trim_end(), "m1 · 2048");
        assert_eq!(state.cells.get("b").unwrap().label.trim_end(), "m2 · 4096");
    }

    #[test]
    fn cell_label_empty_when_nothing_varies() {
        let r = IndicatifReporter::new();
        let p = preview("c", "m", "pr", 1);
        r.suite_started("suite", std::slice::from_ref(&p));
        let state = r.state.lock().unwrap();
        assert_eq!(state.cells.get("c").unwrap().label, "");
    }
}
