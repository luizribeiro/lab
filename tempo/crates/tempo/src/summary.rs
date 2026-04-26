use comfy_table::{presets::UTF8_BORDERS_ONLY, Cell, CellAlignment, ContentArrangement, Table};
use owo_colors::OwoColorize;

use crate::stats::CellStats;

const BAR_WIDTH: usize = 8;
const PARTIAL_BLOCKS: [char; 7] = [
    '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}',
];
const FULL_BLOCK: char = '\u{2588}';
const EM_DASH: &str = "\u{2014}";

fn bar(value: f64, max: f64) -> String {
    if max <= 0.0 || value <= 0.0 {
        return String::new();
    }
    let scaled = (value / max) * BAR_WIDTH as f64;
    let scaled = scaled.min(BAR_WIDTH as f64);
    let full = scaled.floor() as usize;
    let remainder_eighths = ((scaled - full as f64) * 8.0).round() as usize;
    let mut s: String = std::iter::repeat_n(FULL_BLOCK, full).collect();
    if remainder_eighths > 0 && full < BAR_WIDTH {
        s.push(PARTIAL_BLOCKS[remainder_eighths - 1]);
    }
    s
}

fn fmt_ms(value: Option<f64>) -> String {
    match value {
        Some(v) => format!("{} ms", v.round() as i64),
        None => EM_DASH.to_string(),
    }
}

fn fmt_tps(value: Option<f64>) -> String {
    match value {
        Some(v) => format!("{v:.1}"),
        None => EM_DASH.to_string(),
    }
}

fn header_cells(color: bool) -> Vec<Cell> {
    let titles = [
        "scenario",
        "model",
        "prompt",
        "runs",
        "ttft p50",
        "ttft p95",
        "tok/s mean",
        "tok/s",
    ];
    titles
        .iter()
        .map(|t| {
            let text = if color {
                t.bold().cyan().to_string()
            } else {
                (*t).to_string()
            };
            Cell::new(text)
        })
        .collect()
}

pub fn render(stats: &[CellStats], color: bool) -> String {
    let max_decode = stats
        .iter()
        .filter_map(|s| s.decode_tok_s_p50)
        .fold(0.0_f64, f64::max);

    let mut table = Table::new();
    table
        .load_preset(UTF8_BORDERS_ONLY)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(header_cells(color));

    let right = CellAlignment::Right;

    for s in stats {
        let zero_success = s.success_runs == 0;
        let paint = |text: String| -> String {
            if zero_success && color {
                text.red().to_string()
            } else {
                text
            }
        };

        let bar_text = match s.decode_tok_s_p50 {
            Some(v) => bar(v, max_decode),
            None => EM_DASH.to_string(),
        };

        let row = vec![
            Cell::new(paint(s.scenario.clone())),
            Cell::new(paint(s.model.clone())),
            Cell::new(paint(s.prompt.clone())),
            Cell::new(paint(format!("{}/{}", s.success_runs, s.total_runs))).set_alignment(right),
            Cell::new(paint(fmt_ms(s.ttft_ms_p50))).set_alignment(right),
            Cell::new(paint(fmt_ms(s.ttft_ms_p95))).set_alignment(right),
            Cell::new(paint(fmt_tps(s.decode_tok_s_mean))).set_alignment(right),
            Cell::new(paint(bar_text)),
        ];
        table.add_row(row);
    }

    table.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(model: &str, ttft: Option<f64>, decode: Option<f64>, total: u32, ok: u32) -> CellStats {
        CellStats {
            scenario: "decode".into(),
            provider: "p".into(),
            model: model.into(),
            prompt: "short".into(),
            total_runs: total,
            success_runs: ok,
            error_runs: total - ok,
            ttft_ms_p50: ttft,
            ttft_ms_p95: ttft,
            decode_tok_s_mean: decode,
            decode_tok_s_p50: decode,
            decode_tok_s_p95: decode,
            e2e_ms_p50: ttft,
            output_tokens_mean: decode,
        }
    }

    #[test]
    fn no_color_output_has_no_ansi_escapes() {
        let stats = vec![
            cell("m1", Some(100.0), Some(40.0), 3, 3),
            cell("m2", Some(200.0), Some(80.0), 3, 3),
        ];
        let out = render(&stats, false);
        assert!(!out.contains('\x1b'), "found ANSI escape in: {out:?}");
    }

    #[test]
    fn zero_success_row_uses_em_dash_and_zero_over_n() {
        let stats = vec![cell("m1", None, None, 5, 0)];
        let out = render(&stats, false);
        assert!(out.contains("0/5"), "expected 0/5 in: {out}");
        assert!(out.contains(EM_DASH), "expected em-dash in: {out}");
    }

    #[test]
    fn max_row_bar_is_fully_filled() {
        let stats = vec![
            cell("m1", Some(100.0), Some(20.0), 3, 3),
            cell("m2", Some(100.0), Some(80.0), 3, 3),
        ];
        let out = render(&stats, false);
        let full = FULL_BLOCK.to_string().repeat(BAR_WIDTH);
        assert!(
            out.contains(&full),
            "expected fully-filled bar ({full}) in:\n{out}"
        );
    }

    #[test]
    fn header_columns_in_spec_order() {
        let stats = vec![cell("m1", Some(10.0), Some(5.0), 1, 1)];
        let out = render(&stats, false);
        let header_line = out
            .lines()
            .find(|l| l.contains("scenario"))
            .expect("header line");
        let pos = |needle: &str| header_line.find(needle).unwrap_or(usize::MAX);
        let order = [
            pos("scenario"),
            pos("model"),
            pos("prompt"),
            pos("runs"),
            pos("ttft p50"),
            pos("ttft p95"),
            pos("tok/s mean"),
        ];
        for w in order.windows(2) {
            assert!(w[0] < w[1], "columns out of order in:\n{out}");
        }
    }

    #[test]
    fn bar_renders_full_and_partial_blocks() {
        let s = bar(9.0 / 16.0, 1.0);
        let chars: Vec<char> = s.chars().collect();
        assert_eq!(chars.len(), 5);
        assert!(chars[..4].iter().all(|c| *c == FULL_BLOCK));
        assert_eq!(chars[4], PARTIAL_BLOCKS[3]);

        let s = bar(1.0 / 64.0, 1.0);
        assert_eq!(s, PARTIAL_BLOCKS[0].to_string());
    }

    #[test]
    fn bar_zero_max_yields_empty_string() {
        assert_eq!(bar(0.0, 0.0), "");
        assert_eq!(bar(5.0, 0.0), "");
    }
}
