//! Cross-platform inference orchestration: observe the target, compact
//! observed events into a policy, merge with an optional seed config,
//! and write the resulting `lockin.toml`.

use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::process::ExitStatus;

use anyhow::{Context, Result};
use lockin_config::Config;

use crate::compact::{compact, InferredPolicy};
use crate::emit::{merge_into_config_with_read_dirs, render_toml_with_read_dirs};
use lockin_observe::{canonicalize_event, canonicalize_observed, InferDiagnostic, InferEvent};

/// Request describing the command to observe for inference.
#[derive(Debug, Clone)]
pub struct InferRequest {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub current_dir: Option<PathBuf>,
    /// Extra env vars to set on the child (in addition to inherited env).
    pub env: Vec<(OsString, OsString)>,
}

#[derive(Debug)]
pub struct BackendReport {
    pub status: ExitStatus,
    pub events: Vec<InferEvent>,
    pub diagnostics: Vec<InferDiagnostic>,
}

/// Output of an inference run.
#[derive(Debug)]
pub struct InferReport {
    /// The observed program's exit status.
    pub status: ExitStatus,
    /// All raw events the backend produced (after path canonicalization).
    pub events: Vec<InferEvent>,
    /// Diagnostics surfaced by the backend (unsupported ops, malformed lines).
    pub diagnostics: Vec<InferDiagnostic>,
    /// The compacted policy derived from `events`.
    pub policy: InferredPolicy,
    /// The final merged config that was (or would be) serialized to TOML.
    pub config: Config,
}

/// Options for an inference run.
#[derive(Debug, Default)]
pub struct InferOptions {
    /// Optional seed config; observed entries are unioned into its
    /// filesystem section.
    pub seed: Option<Config>,
    /// Where to write the generated TOML. If `None`, the caller is
    /// responsible for using the returned `InferReport.config`.
    pub output: Option<PathBuf>,
}

fn run_observation(req: &InferRequest) -> Result<BackendReport> {
    let raw = lockin_observe::observe_with(
        lockin_observe::ObserveOptions::new(lockin_observe::ObservationKind::InferAllowAll),
        |builder| {
            // POLICY-FREE factory: raw builder + .command, no apply_config.
            let mut cmd = builder
                .command(&req.program)
                .context("building infer sandbox command")?;
            cmd.args(&req.args);
            if let Some(dir) = &req.current_dir {
                cmd.current_dir(dir);
            }
            for (k, v) in &req.env {
                cmd.env(k, v);
            }
            cmd.stdin(std::process::Stdio::null());
            Ok(cmd)
        },
    )?;

    // Strip action — InferReport.events keeps Vec<InferEvent>.
    let mut diagnostics = raw.diagnostics;
    let events: Vec<InferEvent> = raw
        .events
        .into_iter()
        .filter_map(|ae| match canonicalize_event(&ae.event) {
            Ok(ev) => Some(ev),
            Err(d) => {
                diagnostics.push(d);
                None
            }
        })
        .collect();

    Ok(BackendReport {
        status: raw.status,
        events,
        diagnostics,
    })
}

fn finish_infer(
    report: BackendReport,
    options: InferOptions,
    observed_cwd: Option<PathBuf>,
) -> Result<InferReport> {
    let BackendReport {
        status,
        events,
        mut diagnostics,
    } = report;

    let events: Vec<InferEvent> = events
        .into_iter()
        .filter_map(|ev| match canonicalize_event(&ev) {
            Ok(e) => Some(e),
            Err(d) => {
                diagnostics.push(d);
                None
            }
        })
        .collect();

    let policy = compact(&events);
    let extra_read_dirs = observed_cwd.into_iter().collect::<Vec<_>>();
    let config = merge_into_config_with_read_dirs(&policy, options.seed.as_ref(), &extra_read_dirs);

    if let Some(path) = &options.output {
        let body = render_toml_with_read_dirs(&policy, options.seed.as_ref(), &extra_read_dirs)
            .context("rendering inferred lockin.toml")?;
        fs::write(path, body).with_context(|| format!("writing {}", path.display()))?;
    }

    Ok(InferReport {
        status,
        events,
        diagnostics,
        policy,
        config,
    })
}

/// Cross-platform default inference entrypoint.
pub fn infer(request: InferRequest, options: InferOptions) -> Result<InferReport> {
    let report = run_observation(&request)?;
    let observed_cwd = observed_cwd(&request)?;
    finish_infer(report, options, Some(observed_cwd))
}

fn observed_cwd(request: &InferRequest) -> Result<PathBuf> {
    let cwd = match &request.current_dir {
        Some(dir) => dir.clone(),
        None => std::env::current_dir().context("reading current directory for inferred config")?,
    };
    canonicalize_observed(&cwd)
        .with_context(|| format!("canonicalizing observed cwd {}", cwd.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit::HEADER_COMMENT;
    use lockin_observe::{DiagnosticLevel, FsOp};
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    fn infer_from_report(report: BackendReport, options: InferOptions) -> InferReport {
        finish_infer(report, options, None).unwrap()
    }

    fn infer_from_request(
        report: BackendReport,
        options: InferOptions,
        request: &InferRequest,
    ) -> InferReport {
        finish_infer(report, options, Some(observed_cwd(request).unwrap())).unwrap()
    }

    #[test]
    fn events_flow_through_to_policy() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("input.txt");
        std::fs::write(&file, b"x").unwrap();
        let report = BackendReport {
            events: vec![InferEvent::Fs {
                op: FsOp::Read,
                path: file.clone(),
            }],
            diagnostics: vec![],
            status: ExitStatus::from_raw(0),
        };
        let report = infer_from_report(report, InferOptions::default());
        assert!(report.status.success());
        let canon = std::fs::canonicalize(&file).unwrap();
        assert!(
            report.policy.read_paths.contains(&canon),
            "expected {canon:?} in {:?}",
            report.policy.read_paths,
        );
    }

    #[test]
    fn seed_command_is_preserved_and_read_paths_are_unioned() {
        let dir = tempfile::tempdir().unwrap();
        let observed = dir.path().join("observed.txt");
        std::fs::write(&observed, b"x").unwrap();
        let seeded = dir.path().join("seeded.txt");
        std::fs::write(&seeded, b"y").unwrap();
        let canon_seeded = std::fs::canonicalize(&seeded).unwrap();
        let canon_observed = std::fs::canonicalize(&observed).unwrap();

        let seed = Config {
            command: Some(vec!["/usr/bin/python3".into(), "-u".into()]),
            filesystem: lockin_config::FilesystemConfig {
                read_paths: vec![canon_seeded.clone()],
                ..Default::default()
            },
            ..Default::default()
        };

        let report = BackendReport {
            events: vec![InferEvent::Fs {
                op: FsOp::Read,
                path: observed,
            }],
            diagnostics: vec![],
            status: ExitStatus::from_raw(0),
        };
        let opts = InferOptions {
            seed: Some(seed),
            output: None,
        };
        let report = infer_from_report(report, opts);
        assert_eq!(
            report.config.command,
            Some(vec!["/usr/bin/python3".into(), "-u".into()])
        );
        assert!(report.config.filesystem.read_paths.contains(&canon_seeded));
        assert!(report
            .config
            .filesystem
            .read_paths
            .contains(&canon_observed));
    }

    #[test]
    fn output_file_is_written_when_path_given() {
        let dir = tempfile::tempdir().unwrap();
        let observed = dir.path().join("input.txt");
        std::fs::write(&observed, b"x").unwrap();
        let out = dir.path().join("lockin.toml");

        let report = BackendReport {
            events: vec![InferEvent::Fs {
                op: FsOp::Read,
                path: observed.clone(),
            }],
            diagnostics: vec![],
            status: ExitStatus::from_raw(0),
        };
        let opts = InferOptions {
            seed: None,
            output: Some(out.clone()),
        };
        let _report = infer_from_report(report, opts);

        let body = std::fs::read_to_string(&out).unwrap();
        assert!(body.starts_with(HEADER_COMMENT), "missing header:\n{body}");
        let canon = std::fs::canonicalize(&observed).unwrap();
        assert!(
            body.contains(canon.to_str().unwrap()),
            "missing observed path in:\n{body}"
        );
    }

    #[test]
    fn request_cwd_is_added_to_read_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let request = InferRequest {
            program: PathBuf::from("/usr/bin/true"),
            args: vec![],
            current_dir: Some(dir.path().to_path_buf()),
            env: vec![],
        };
        let report = BackendReport {
            events: vec![],
            diagnostics: vec![],
            status: ExitStatus::from_raw(0),
        };

        let report = infer_from_request(report, InferOptions::default(), &request);
        let cwd = std::fs::canonicalize(dir.path()).unwrap();
        assert!(
            report.config.filesystem.read_dirs.contains(&cwd),
            "expected cwd {cwd:?} in {:?}",
            report.config.filesystem.read_dirs
        );
        assert!(
            report.config.filesystem.write_dirs.is_empty(),
            "cwd must not be added as writable: {:?}",
            report.config.filesystem.write_dirs
        );
    }

    #[test]
    fn diagnostics_propagate() {
        let report = BackendReport {
            events: vec![],
            diagnostics: vec![InferDiagnostic {
                level: DiagnosticLevel::Warn,
                message: "test diagnostic".into(),
            }],
            status: ExitStatus::from_raw(0),
        };
        let report = infer_from_report(report, InferOptions::default());
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].message, "test diagnostic");
    }

    #[test]
    fn nonzero_status_propagates() {
        let report = BackendReport {
            events: vec![],
            diagnostics: vec![],
            status: ExitStatus::from_raw(1 << 8),
        };
        let report = infer_from_report(report, InferOptions::default());
        assert!(!report.status.success());
        assert_eq!(report.status.code(), Some(1));
    }

    #[test]
    fn uncanonicalizable_path_becomes_diagnostic_and_is_dropped() {
        let bad = PathBuf::from("/tmp/path-with-control\x01-char");
        let report = BackendReport {
            events: vec![InferEvent::Fs {
                op: FsOp::Read,
                path: bad,
            }],
            diagnostics: vec![],
            status: ExitStatus::from_raw(0),
        };
        let report = infer_from_report(report, InferOptions::default());
        assert!(report.events.is_empty());
        assert_eq!(report.diagnostics.len(), 1);
        assert!(report.diagnostics[0].message.contains("dropping fs event"));
    }
}
