//! Cross-platform inference orchestration: choose the right backend, run
//! the target, compact observed events into a policy, merge with an
//! optional seed config, and write the resulting `lockin.toml`.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use lockin_config::Config;

use crate::backend::{BackendReport, InferBackend, InferRequest};
use crate::compact::{compact, InferredPolicy};
use crate::emit::{merge_into_config, render_toml};
use lockin_observe::{canonicalize_event, InferDiagnostic, InferEvent};

/// Output of an inference run.
#[derive(Debug)]
pub struct InferReport {
    /// The observed program's exit status.
    pub status: std::process::ExitStatus,
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

/// Run inference using a specific backend.
pub fn infer_with_backend<B: InferBackend>(
    backend: &B,
    request: InferRequest,
    options: InferOptions,
) -> Result<InferReport> {
    let BackendReport {
        status,
        events,
        mut diagnostics,
    } = backend.run(&request)?;

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
    let config = merge_into_config(&policy, options.seed.as_ref());

    if let Some(path) = &options.output {
        let body = render_toml(&policy, options.seed.as_ref())
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

/// Cross-platform default: pick the right backend for the host OS.
pub fn infer(request: InferRequest, options: InferOptions) -> Result<InferReport> {
    #[cfg(target_os = "linux")]
    {
        infer_with_backend(&crate::backend::linux::LinuxBackend, request, options)
    }
    #[cfg(target_os = "macos")]
    {
        infer_with_backend(&crate::backend::darwin::DarwinBackend, request, options)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = (request, options);
        anyhow::bail!("lockin infer is not implemented for this platform")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emit::HEADER_COMMENT;
    use lockin_observe::{DiagnosticLevel, FsOp};
    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;

    struct TestBackend {
        events: Vec<InferEvent>,
        diagnostics: Vec<InferDiagnostic>,
        status: ExitStatus,
    }

    impl InferBackend for TestBackend {
        fn run(&self, _request: &InferRequest) -> Result<BackendReport> {
            Ok(BackendReport {
                status: self.status,
                events: self.events.clone(),
                diagnostics: self.diagnostics.clone(),
            })
        }
    }

    fn req() -> InferRequest {
        InferRequest {
            program: PathBuf::from("/bin/true"),
            args: vec![],
            current_dir: None,
            env: vec![],
        }
    }

    #[test]
    fn events_flow_through_to_policy() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("input.txt");
        std::fs::write(&file, b"x").unwrap();
        let backend = TestBackend {
            events: vec![InferEvent::Fs {
                op: FsOp::Read,
                path: file.clone(),
            }],
            diagnostics: vec![],
            status: ExitStatus::from_raw(0),
        };
        let report = infer_with_backend(&backend, req(), InferOptions::default()).unwrap();
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

        let backend = TestBackend {
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
        let report = infer_with_backend(&backend, req(), opts).unwrap();
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

        let backend = TestBackend {
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
        let _report = infer_with_backend(&backend, req(), opts).unwrap();

        let body = std::fs::read_to_string(&out).unwrap();
        assert!(body.starts_with(HEADER_COMMENT), "missing header:\n{body}");
        let canon = std::fs::canonicalize(&observed).unwrap();
        assert!(
            body.contains(canon.to_str().unwrap()),
            "missing observed path in:\n{body}"
        );
    }

    #[test]
    fn diagnostics_propagate() {
        let backend = TestBackend {
            events: vec![],
            diagnostics: vec![InferDiagnostic {
                level: DiagnosticLevel::Warn,
                message: "test diagnostic".into(),
            }],
            status: ExitStatus::from_raw(0),
        };
        let report = infer_with_backend(&backend, req(), InferOptions::default()).unwrap();
        assert_eq!(report.diagnostics.len(), 1);
        assert_eq!(report.diagnostics[0].message, "test diagnostic");
    }

    #[test]
    fn nonzero_status_propagates() {
        let backend = TestBackend {
            events: vec![],
            diagnostics: vec![],
            status: ExitStatus::from_raw(1 << 8),
        };
        let report = infer_with_backend(&backend, req(), InferOptions::default()).unwrap();
        assert!(!report.status.success());
        assert_eq!(report.status.code(), Some(1));
    }

    #[test]
    fn uncanonicalizable_path_becomes_diagnostic_and_is_dropped() {
        let bad = PathBuf::from("/tmp/path-with-control\x01-char");
        let backend = TestBackend {
            events: vec![InferEvent::Fs {
                op: FsOp::Read,
                path: bad,
            }],
            diagnostics: vec![],
            status: ExitStatus::from_raw(0),
        };
        let report = infer_with_backend(&backend, req(), InferOptions::default()).unwrap();
        assert!(report.events.is_empty());
        assert_eq!(report.diagnostics.len(), 1);
        assert!(report.diagnostics[0].message.contains("dropping fs event"));
    }
}
