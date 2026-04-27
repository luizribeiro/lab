use comfy_table::{presets::UTF8_BORDERS_ONLY, Cell, CellAlignment, ContentArrangement, Table};
use owo_colors::OwoColorize;

use crate::dimensions::Dimensions;
use crate::stats::CellStats;

const EM_DASH: &str = "\u{2014}";

const METRIC_TITLES: [&str; 5] = ["runs", "ttft", "tok/s", "tokens", "total"];

fn fmt_ttft(mean: Option<f64>, stddev: Option<f64>) -> String {
    match mean {
        Some(m) => match stddev {
            Some(sd) => format!("{} \u{00b1} {} ms", m.round() as i64, sd.round() as i64),
            None => format!("{} ms", m.round() as i64),
        },
        None => EM_DASH.to_string(),
    }
}

fn fmt_tps(mean: Option<f64>, stddev: Option<f64>) -> String {
    match mean {
        Some(m) => match stddev {
            Some(sd) => format!("{m:.1} \u{00b1} {sd:.1}"),
            None => format!("{m:.1}"),
        },
        None => EM_DASH.to_string(),
    }
}

fn fmt_tokens(value: Option<f64>) -> String {
    match value {
        Some(v) => format!("{}", v.round() as i64),
        None => EM_DASH.to_string(),
    }
}

fn format_duration(total_ms: f64) -> String {
    let total_secs = (total_ms / 1000.0).round() as i64;
    if total_secs < 60 {
        format!("{total_secs}s")
    } else if total_secs < 3600 {
        let m = total_secs / 60;
        let s = total_secs % 60;
        format!("{m}m {s}s")
    } else {
        let h = total_secs / 3600;
        let m = (total_secs % 3600) / 60;
        format!("{h}h {m}m")
    }
}

fn fmt_total(value: Option<f64>) -> String {
    match value {
        Some(v) => format_duration(v),
        None => EM_DASH.to_string(),
    }
}

fn header_cells(varying: &[&str], color: bool) -> Vec<Cell> {
    varying
        .iter()
        .copied()
        .chain(METRIC_TITLES.iter().copied())
        .map(|t| {
            let text = if color {
                t.bold().cyan().to_string()
            } else {
                t.to_string()
            };
            Cell::new(text)
        })
        .collect()
}

pub fn render(stats: &[CellStats], color: bool) -> String {
    let dims: Vec<Dimensions> = stats.iter().map(|s| s.dimensions.clone()).collect();
    let varying = Dimensions::varying(&dims);

    let mut table = Table::new();
    table
        .load_preset(UTF8_BORDERS_ONLY)
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_header(header_cells(&varying, color));

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

        let mut row: Vec<Cell> = varying
            .iter()
            .map(|axis| Cell::new(paint(s.dimensions.axis_value(axis))))
            .collect();
        row.extend([
            Cell::new(paint(format!("{}/{}", s.success_runs, s.total_runs))).set_alignment(right),
            Cell::new(paint(fmt_ttft(s.ttft_ms_p50, s.ttft_ms_stddev))).set_alignment(right),
            Cell::new(paint(fmt_tps(s.decode_tok_s_mean, s.decode_tok_s_stddev)))
                .set_alignment(right),
            Cell::new(paint(fmt_tokens(s.output_tokens_mean))).set_alignment(right),
            Cell::new(paint(fmt_total(s.e2e_ms_total))).set_alignment(right),
        ]);
        table.add_row(row);
    }

    table.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::var::VarValue;
    use indexmap::IndexMap;

    fn cell_with(
        dimensions: Dimensions,
        ttft: Option<f64>,
        decode: Option<f64>,
        total: u32,
        ok: u32,
    ) -> CellStats {
        CellStats {
            dimensions,
            total_runs: total,
            success_runs: ok,
            error_runs: total - ok,
            ttft_ms_p50: ttft,
            ttft_ms_stddev: None,
            decode_tok_s_mean: decode,
            decode_tok_s_stddev: None,
            decode_tok_s_p50: decode,
            e2e_ms_p50: ttft,
            e2e_ms_total: ttft,
            output_tokens_mean: decode,
        }
    }

    fn dim(scenario: &str, provider: &str, vars: &[(&str, VarValue)]) -> Dimensions {
        let mut m: IndexMap<String, VarValue> = IndexMap::new();
        for (k, v) in vars {
            m.insert((*k).to_owned(), v.clone());
        }
        Dimensions {
            scenario: scenario.to_owned(),
            provider: provider.to_owned(),
            vars: m,
        }
    }

    fn cell(model: &str, ttft: Option<f64>, decode: Option<f64>, total: u32, ok: u32) -> CellStats {
        cell_with(
            crate::dimensions::test_dimensions("decode", "p", model, "short"),
            ttft,
            decode,
            total,
            ok,
        )
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
    fn only_varying_axis_appears_in_header() {
        let stats = vec![
            cell("m1", Some(10.0), Some(5.0), 1, 1),
            cell("m2", Some(20.0), Some(6.0), 1, 1),
        ];
        let out = render(&stats, false);
        let header_line = out.lines().find(|l| l.contains("model")).expect("header");
        assert!(header_line.contains("model"));
        assert!(!header_line.contains("scenario"));
        assert!(!header_line.contains("prompt"));
        assert!(!header_line.contains("provider"));
    }

    #[test]
    fn multi_axis_columns_in_declaration_order() {
        let a = dim(
            "decode",
            "litellm",
            &[
                ("model", VarValue::from("gpt")),
                ("max_tokens", VarValue::from(2048i64)),
            ],
        );
        let b = dim(
            "decode",
            "litellm",
            &[
                ("model", VarValue::from("claude")),
                ("max_tokens", VarValue::from(4096i64)),
            ],
        );
        let stats = vec![
            cell_with(a, Some(10.0), Some(5.0), 1, 1),
            cell_with(b, Some(20.0), Some(6.0), 1, 1),
        ];
        let out = render(&stats, false);
        let header_line = out.lines().find(|l| l.contains("model")).expect("header");
        let m = header_line.find("model").unwrap();
        let t = header_line.find("max_tokens").unwrap();
        let r = header_line.find("runs").unwrap();
        assert!(m < t && t < r, "columns out of order in:\n{out}");
        assert!(!header_line.contains("scenario"));
    }

    #[test]
    fn all_constant_renders_only_metric_columns() {
        let stats = vec![cell("m1", Some(10.0), Some(5.0), 1, 1)];
        let out = render(&stats, false);
        let header_line = out.lines().find(|l| l.contains("runs")).expect("header");
        assert!(!header_line.contains("model"));
        assert!(!header_line.contains("scenario"));
        assert!(!header_line.contains("prompt"));
        for title in METRIC_TITLES {
            assert!(header_line.contains(title), "missing {title} in:\n{out}");
        }
    }

    #[test]
    fn no_bar_chart_blocks_in_output() {
        let stats = vec![
            cell("m1", Some(100.0), Some(20.0), 3, 3),
            cell("m2", Some(100.0), Some(80.0), 3, 3),
        ];
        let out = render(&stats, false);
        for ch in [
            '\u{2588}', '\u{2587}', '\u{2586}', '\u{2585}', '\u{2584}', '\u{2583}', '\u{2582}',
            '\u{2581}',
        ] {
            assert!(
                !out.contains(ch),
                "found bar-chart block char {ch:?} in:\n{out}"
            );
        }
    }

    #[test]
    fn no_p95_column_in_header() {
        let stats = vec![cell("m1", Some(10.0), Some(5.0), 3, 3)];
        let out = render(&stats, false);
        assert!(!out.contains("p95"), "p95 column should be gone:\n{out}");
    }

    #[test]
    fn ttft_with_stddev_renders_plus_minus() {
        let mut s = cell("m1", Some(158.0), Some(47.3), 5, 5);
        s.ttft_ms_stddev = Some(4.0);
        s.decode_tok_s_stddev = Some(0.6);
        let out = render(&[s], false);
        assert!(out.contains("158 \u{00b1} 4 ms"), "ttft format in:\n{out}");
        assert!(out.contains("47.3 \u{00b1} 0.6"), "tok/s format in:\n{out}");
    }

    #[test]
    fn ttft_without_stddev_renders_plain() {
        let s = cell("m1", Some(158.0), Some(47.3), 1, 1);
        let out = render(&[s], false);
        assert!(out.contains("158 ms"));
        assert!(!out.contains("\u{00b1}"));
    }

    #[test]
    fn tokens_column_shows_rounded_mean() {
        let mut s = cell("m1", Some(100.0), Some(40.0), 1, 1);
        s.output_tokens_mean = Some(2048.4);
        let out = render(&[s], false);
        assert!(out.contains("2048"), "tokens cell in:\n{out}");
    }

    #[test]
    fn total_column_formats_duration() {
        let mut s = cell("m1", Some(100.0), Some(40.0), 5, 5);
        s.e2e_ms_total = Some(216_000.0); // 3m 36s
        let out = render(&[s], false);
        assert!(out.contains("3m 36s"), "expected duration in:\n{out}");
    }

    #[test]
    fn format_duration_branches() {
        assert_eq!(format_duration(12_000.0), "12s");
        assert_eq!(format_duration(216_000.0), "3m 36s");
        assert_eq!(format_duration(45_000.0), "45s");
        assert_eq!(format_duration(4_320_000.0), "1h 12m");
    }
}
