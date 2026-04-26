use crate::provider::metrics::Run;

#[derive(Debug, Clone, PartialEq)]
pub struct CellStats {
    pub scenario: String,
    pub provider: String,
    pub model: String,
    pub prompt: String,
    pub total_runs: u32,
    pub success_runs: u32,
    pub error_runs: u32,
    pub ttft_ms_p50: Option<f64>,
    pub ttft_ms_p95: Option<f64>,
    pub decode_tok_s_mean: Option<f64>,
    pub decode_tok_s_p50: Option<f64>,
    pub decode_tok_s_p95: Option<f64>,
    pub e2e_ms_p50: Option<f64>,
    pub output_tokens_mean: Option<f64>,
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let n = sorted.len();
    if n == 1 {
        return sorted[0];
    }
    let idx = (n as f64 - 1.0) * p;
    let lo = idx.floor() as usize;
    let hi = idx.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let frac = idx - lo as f64;
        sorted[lo] + frac * (sorted[hi] - sorted[lo])
    }
}

fn collect<F: Fn(&Run) -> Option<f64>>(runs: &[&Run], extract: F) -> Vec<f64> {
    let mut v: Vec<f64> = runs.iter().filter_map(|r| extract(r)).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v
}

fn mean(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().sum::<f64>() / values.len() as f64)
    }
}

fn p(sorted: &[f64], pct: f64) -> Option<f64> {
    if sorted.is_empty() {
        None
    } else {
        Some(percentile(sorted, pct))
    }
}

pub fn aggregate(runs: &[Run]) -> Vec<CellStats> {
    let mut groups: Vec<(String, String, String, String, Vec<&Run>)> = Vec::new();

    for run in runs {
        let key = (
            run.scenario.clone(),
            run.provider.clone(),
            run.model.clone(),
            run.prompt.clone(),
        );
        if let Some(existing) = groups
            .iter_mut()
            .find(|g| g.0 == key.0 && g.1 == key.1 && g.2 == key.2 && g.3 == key.3)
        {
            existing.4.push(run);
        } else {
            groups.push((key.0, key.1, key.2, key.3, vec![run]));
        }
    }

    groups
        .into_iter()
        .map(|(scenario, provider, model, prompt, cell_runs)| {
            let total_runs = cell_runs.len() as u32;
            let successes: Vec<&Run> = cell_runs
                .iter()
                .copied()
                .filter(|r| r.error.is_none())
                .collect();
            let success_runs = successes.len() as u32;
            let error_runs = total_runs - success_runs;

            let ttft = collect(&successes, |r| r.ttft_ms);
            let decode = collect(&successes, |r| r.decode_tok_s);
            let e2e = collect(&successes, |r| r.e2e_ms);
            let out_tok: Vec<f64> = successes
                .iter()
                .filter_map(|r| r.output_tokens.map(|t| t as f64))
                .collect();

            CellStats {
                scenario,
                provider,
                model,
                prompt,
                total_runs,
                success_runs,
                error_runs,
                ttft_ms_p50: p(&ttft, 0.50),
                ttft_ms_p95: p(&ttft, 0.95),
                decode_tok_s_mean: mean(&decode),
                decode_tok_s_p50: p(&decode, 0.50),
                decode_tok_s_p95: p(&decode, 0.95),
                e2e_ms_p50: p(&e2e, 0.50),
                output_tokens_mean: mean(&out_tok),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn run(model: &str, ttft: Option<f64>, decode: Option<f64>, error: Option<&str>) -> Run {
        Run {
            suite: "s".into(),
            scenario: "decode".into(),
            provider: "p".into(),
            model: model.into(),
            prompt: "short".into(),
            run_idx: 0,
            started_at: Utc::now(),
            ttft_ms: ttft,
            decode_tok_s: decode,
            e2e_ms: ttft,
            input_tokens: Some(10),
            output_tokens: decode.map(|d| d as u64),
            error: error.map(|s| s.into()),
        }
    }

    #[test]
    fn percentile_known_5_element_sample() {
        let xs = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((percentile(&xs, 0.50) - 3.0).abs() < 1e-9);
        assert!((percentile(&xs, 0.95) - 4.8).abs() < 1e-9);
    }

    #[test]
    fn aggregate_5_runs_one_cell() {
        let runs: Vec<Run> = (1..=5)
            .map(|i| run("m1", Some(i as f64), Some(i as f64), None))
            .collect();
        let stats = aggregate(&runs);
        assert_eq!(stats.len(), 1);
        let s = &stats[0];
        assert_eq!(s.total_runs, 5);
        assert_eq!(s.success_runs, 5);
        assert_eq!(s.error_runs, 0);
        assert!((s.ttft_ms_p50.unwrap() - 3.0).abs() < 1e-9);
        assert!((s.ttft_ms_p95.unwrap() - 4.8).abs() < 1e-9);
        assert!((s.decode_tok_s_mean.unwrap() - 3.0).abs() < 1e-9);
    }

    #[test]
    fn errors_excluded_from_numeric_stats() {
        let runs = vec![
            run("m1", Some(10.0), Some(50.0), None),
            run("m1", Some(20.0), Some(60.0), None),
            run("m1", None, None, Some("http_500")),
        ];
        let stats = aggregate(&runs);
        let s = &stats[0];
        assert_eq!(s.total_runs, 3);
        assert_eq!(s.success_runs, 2);
        assert_eq!(s.error_runs, 1);
        assert!((s.ttft_ms_p50.unwrap() - 15.0).abs() < 1e-9);
        assert!((s.decode_tok_s_mean.unwrap() - 55.0).abs() < 1e-9);
    }

    #[test]
    fn single_run_cell() {
        let runs = vec![run("m1", Some(42.0), Some(7.0), None)];
        let stats = aggregate(&runs);
        let s = &stats[0];
        assert_eq!(s.success_runs, 1);
        assert!((s.ttft_ms_p50.unwrap() - 42.0).abs() < 1e-9);
        assert!((s.ttft_ms_p95.unwrap() - 42.0).abs() < 1e-9);
    }

    #[test]
    fn all_error_cell_has_none_stats() {
        let runs = vec![
            run("m1", None, None, Some("http_500")),
            run("m1", None, None, Some("timeout")),
        ];
        let stats = aggregate(&runs);
        let s = &stats[0];
        assert_eq!(s.total_runs, 2);
        assert_eq!(s.success_runs, 0);
        assert_eq!(s.error_runs, 2);
        assert!(s.ttft_ms_p50.is_none());
        assert!(s.ttft_ms_p95.is_none());
        assert!(s.decode_tok_s_mean.is_none());
        assert!(s.decode_tok_s_p50.is_none());
        assert!(s.decode_tok_s_p95.is_none());
        assert!(s.e2e_ms_p50.is_none());
        assert!(s.output_tokens_mean.is_none());
    }

    #[test]
    fn groups_by_scenario_provider_model_prompt() {
        let runs = vec![
            run("m1", Some(1.0), Some(1.0), None),
            run("m2", Some(2.0), Some(2.0), None),
            run("m1", Some(3.0), Some(3.0), None),
        ];
        let stats = aggregate(&runs);
        assert_eq!(stats.len(), 2);
        let m1 = stats.iter().find(|s| s.model == "m1").unwrap();
        let m2 = stats.iter().find(|s| s.model == "m2").unwrap();
        assert_eq!(m1.success_runs, 2);
        assert_eq!(m2.success_runs, 1);
    }
}
