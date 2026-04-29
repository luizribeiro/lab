//! Parser for macOS Seatbelt sandbox report messages.

use std::path::PathBuf;

use crate::event::{DiagnosticLevel, FsOp, InferDiagnostic, InferEvent};

/// Outcome of parsing one Seatbelt eventMessage string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeatbeltParseOutcome {
    /// Successfully classified as an inference event tagged with our RUN_ID.
    Event(InferEvent),
    /// Recognized as a Seatbelt report but not relevant (different RUN_ID,
    /// duplicate-compression line, allow process-fork without a path,
    /// etc.). Skip silently.
    Skip,
    /// Recognized as a Seatbelt operation but not translatable (mach-lookup,
    /// sysctl-read, file-ioctl, network*, etc.). Surfaces as a diagnostic.
    Unsupported(InferDiagnostic),
    /// Line did not match the expected Seatbelt grammar.
    Malformed(InferDiagnostic),
}

const BACKEND: &str = "seatbelt";

/// Parse one Seatbelt eventMessage. `expected_run_id` is the per-run UUID
/// the caller embedded into the sandbox profile via
/// `(with message (param "RUN_ID"))`.
/// Messages tagged with a different (or missing) RUN_ID return `Skip`.
pub fn parse_message(message: &str, expected_run_id: &str) -> SeatbeltParseOutcome {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return SeatbeltParseOutcome::Skip;
    }

    if is_duplicate_report(trimmed) {
        return SeatbeltParseOutcome::Skip;
    }

    let lines: Vec<&str> = trimmed
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    if lines.len() < 2 {
        return SeatbeltParseOutcome::Skip;
    }

    let run_id_line = lines.last().copied().unwrap_or("");
    if run_id_line != expected_run_id {
        return SeatbeltParseOutcome::Skip;
    }

    let first = lines[0];
    let Some(rest) = first.strip_prefix("Sandbox: ") else {
        return malformed(format!("expected 'Sandbox: ' prefix: {first}"));
    };

    let Some((_proc, after_proc)) = parse_process(rest) else {
        return malformed(format!(
            "missing or invalid '<name>(<pid>)' header: {first}"
        ));
    };

    let mut tokens = after_proc.splitn(3, char::is_whitespace);
    let action = tokens.next().unwrap_or("");
    let operation = tokens.next().unwrap_or("");
    let remainder = tokens.next().unwrap_or("").trim();

    if action.is_empty() || operation.is_empty() {
        return malformed(format!("missing action/operation: {first}"));
    }

    classify(operation, remainder, first)
}

fn is_duplicate_report(s: &str) -> bool {
    let mut iter = s.splitn(2, ' ');
    let first = iter.next().unwrap_or("");
    let after = iter.next().unwrap_or("");
    first.parse::<u64>().is_ok() && after.starts_with("duplicate report for ")
}

/// Parse `<name>(<pid>)` at the start of `s`. Returns (name, remainder
/// after the closing `)` with leading whitespace stripped).
fn parse_process(s: &str) -> Option<(&str, &str)> {
    let lparen = s.find('(')?;
    let rparen_offset_in_rest = s[lparen + 1..].find(')')?;
    let rparen = lparen + 1 + rparen_offset_in_rest;
    let name = &s[..lparen];
    let pid_str = &s[lparen + 1..rparen];
    pid_str.parse::<u64>().ok()?;
    let after = s[rparen + 1..].trim_start();
    Some((name, after))
}

fn classify(op: &str, remainder: &str, first_line: &str) -> SeatbeltParseOutcome {
    let path_op = |fs_op: FsOp| -> SeatbeltParseOutcome {
        if remainder.is_empty() {
            return malformed(format!("{op}: missing path in {first_line}"));
        }
        SeatbeltParseOutcome::Event(InferEvent::Fs {
            op: fs_op,
            path: PathBuf::from(remainder),
        })
    };

    match op {
        "file-read-data" | "file-read-metadata" => path_op(FsOp::Read),
        "file-readdir" => path_op(FsOp::ReadDir),
        "file-write-data"
        | "file-write-mount"
        | "file-write-mode"
        | "file-write-owner"
        | "file-write-times"
        | "file-write-flags"
        | "file-write-finderinfo"
        | "file-write-setugid"
        | "file-write-xattr" => path_op(FsOp::Write),
        "file-write-create" => path_op(FsOp::Create),
        "file-write-unlink" => path_op(FsOp::Delete),
        "process-exec*" | "process-exec" | "process-exec-interpreter" => {
            if remainder.is_empty() {
                return malformed(format!("{op}: missing path in {first_line}"));
            }
            SeatbeltParseOutcome::Event(InferEvent::Exec {
                path: PathBuf::from(remainder),
            })
        }
        "process-fork" => SeatbeltParseOutcome::Skip,
        _ => SeatbeltParseOutcome::Unsupported(diag(
            DiagnosticLevel::Warn,
            format!("{BACKEND}: operation {op:?} has no lockin schema mapping ({first_line})"),
        )),
    }
}

fn malformed(message: String) -> SeatbeltParseOutcome {
    SeatbeltParseOutcome::Malformed(diag(DiagnosticLevel::Warn, format!("{BACKEND}: {message}")))
}

fn diag(level: DiagnosticLevel, message: String) -> InferDiagnostic {
    InferDiagnostic { level, message }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUN_ID: &str = "lockin-run-3A17723F-2EED-459D-9350-E62638EBCB05";

    fn msg(first: &str) -> String {
        format!("{first}\n{RUN_ID}")
    }

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn file_read_data_maps_to_read() {
        assert_eq!(
            parse_message(
                &msg("Sandbox: coreutils(3053) allow file-read-data /private/etc/hosts"),
                RUN_ID
            ),
            SeatbeltParseOutcome::Event(InferEvent::Fs {
                op: FsOp::Read,
                path: p("/private/etc/hosts"),
            })
        );
    }

    #[test]
    fn file_read_metadata_promotes_to_read() {
        assert!(matches!(
            parse_message(
                &msg("Sandbox: bash(1) allow file-read-metadata /etc/hosts"),
                RUN_ID
            ),
            SeatbeltParseOutcome::Event(InferEvent::Fs { op: FsOp::Read, .. })
        ));
    }

    #[test]
    fn file_readdir_maps_to_readdir() {
        assert!(matches!(
            parse_message(&msg("Sandbox: ls(1) allow file-readdir /etc"), RUN_ID),
            SeatbeltParseOutcome::Event(InferEvent::Fs {
                op: FsOp::ReadDir,
                ..
            })
        ));
    }

    #[test]
    fn file_write_create_maps_to_create() {
        assert_eq!(
            parse_message(
                &msg("Sandbox: bash(3052) allow file-write-create /private/tmp/lockin-audit-q2-file3"),
                RUN_ID
            ),
            SeatbeltParseOutcome::Event(InferEvent::Fs {
                op: FsOp::Create,
                path: p("/private/tmp/lockin-audit-q2-file3"),
            })
        );
    }

    #[test]
    fn file_write_data_maps_to_write() {
        assert!(matches!(
            parse_message(
                &msg("Sandbox: bash(1) allow file-write-data /tmp/out"),
                RUN_ID
            ),
            SeatbeltParseOutcome::Event(InferEvent::Fs {
                op: FsOp::Write,
                ..
            })
        ));
    }

    #[test]
    fn file_write_mode_owner_xattr_etc_all_map_to_write() {
        for op in [
            "file-write-mode",
            "file-write-owner",
            "file-write-times",
            "file-write-flags",
            "file-write-finderinfo",
            "file-write-setugid",
            "file-write-xattr",
            "file-write-mount",
        ] {
            let msg = msg(&format!("Sandbox: bash(1) allow {op} /tmp/x"));
            assert!(
                matches!(
                    parse_message(&msg, RUN_ID),
                    SeatbeltParseOutcome::Event(InferEvent::Fs {
                        op: FsOp::Write,
                        ..
                    })
                ),
                "operation {op}"
            );
        }
    }

    #[test]
    fn file_write_unlink_maps_to_delete() {
        assert!(matches!(
            parse_message(
                &msg("Sandbox: bash(1) allow file-write-unlink /tmp/old"),
                RUN_ID
            ),
            SeatbeltParseOutcome::Event(InferEvent::Fs {
                op: FsOp::Delete,
                ..
            })
        ));
    }

    #[test]
    fn process_exec_star_maps_to_exec() {
        assert_eq!(
            parse_message(
                &msg("Sandbox: sandbox-exec(3052) allow process-exec* /bin/sh"),
                RUN_ID
            ),
            SeatbeltParseOutcome::Event(InferEvent::Exec { path: p("/bin/sh") })
        );
    }

    #[test]
    fn process_exec_interpreter_maps_to_exec() {
        assert!(matches!(
            parse_message(
                &msg("Sandbox: sh(1) allow process-exec-interpreter /usr/bin/perl"),
                RUN_ID
            ),
            SeatbeltParseOutcome::Event(InferEvent::Exec { .. })
        ));
    }

    #[test]
    fn process_fork_is_skip() {
        assert_eq!(
            parse_message(&msg("Sandbox: bash(3052) allow process-fork"), RUN_ID),
            SeatbeltParseOutcome::Skip
        );
    }

    #[test]
    fn mach_lookup_is_unsupported() {
        let outcome = parse_message(
            &msg("Sandbox: bash(1) allow mach-lookup com.apple.foo"),
            RUN_ID,
        );
        let SeatbeltParseOutcome::Unsupported(d) = outcome else {
            panic!("expected Unsupported, got {outcome:?}");
        };
        assert!(d.message.contains("mach-lookup"));
    }

    #[test]
    fn sysctl_read_is_unsupported() {
        let outcome = parse_message(
            &msg("Sandbox: bash(2901) allow sysctl-read kern.bootargs"),
            RUN_ID,
        );
        let SeatbeltParseOutcome::Unsupported(d) = outcome else {
            panic!("expected Unsupported, got {outcome:?}");
        };
        assert!(d.message.contains("sysctl-read"));
    }

    #[test]
    fn network_outbound_is_unsupported() {
        let outcome = parse_message(
            &msg("Sandbox: bash(1) allow network-outbound remote:*:443"),
            RUN_ID,
        );
        let SeatbeltParseOutcome::Unsupported(d) = outcome else {
            panic!("expected Unsupported, got {outcome:?}");
        };
        assert!(d.message.contains("network-outbound"));
    }

    #[test]
    fn file_ioctl_is_unsupported() {
        let outcome = parse_message(
            &msg(
                "Sandbox: bash(3052) allow file-ioctl path:/dev/dtracehelper ioctl-command:(_IO \"h\" 4)",
            ),
            RUN_ID,
        );
        assert!(matches!(outcome, SeatbeltParseOutcome::Unsupported(_)));
    }

    #[test]
    fn different_run_id_is_skip() {
        let other = "Sandbox: bash(1) allow file-read-data /etc/hosts\nlockin-run-OTHER-RUN-ID";
        assert_eq!(parse_message(other, RUN_ID), SeatbeltParseOutcome::Skip);
    }

    #[test]
    fn missing_run_id_line_is_skip() {
        let only = "Sandbox: bash(1) allow file-read-data /etc/hosts";
        assert_eq!(parse_message(only, RUN_ID), SeatbeltParseOutcome::Skip);
    }

    #[test]
    fn duplicate_report_line_is_skip() {
        let dup = "1 duplicate report for Sandbox: bash(2901) allow file-read-data /bin/bash";
        assert_eq!(parse_message(dup, RUN_ID), SeatbeltParseOutcome::Skip);
    }

    #[test]
    fn double_digit_duplicate_report_line_is_skip() {
        let dup = "37 duplicate report for Sandbox: bash(2901) allow file-read-data /bin/bash";
        assert_eq!(parse_message(dup, RUN_ID), SeatbeltParseOutcome::Skip);
    }

    #[test]
    fn empty_message_is_skip() {
        assert_eq!(parse_message("", RUN_ID), SeatbeltParseOutcome::Skip);
        assert_eq!(parse_message("   \n\n", RUN_ID), SeatbeltParseOutcome::Skip);
    }

    #[test]
    fn missing_sandbox_prefix_is_malformed() {
        let m = format!("notasandboxline\n{RUN_ID}");
        assert!(matches!(
            parse_message(&m, RUN_ID),
            SeatbeltParseOutcome::Malformed(_)
        ));
    }

    #[test]
    fn missing_pid_parens_is_malformed() {
        let m = format!("Sandbox: bash allow file-read-data /etc/hosts\n{RUN_ID}");
        assert!(matches!(
            parse_message(&m, RUN_ID),
            SeatbeltParseOutcome::Malformed(_)
        ));
    }

    #[test]
    fn nonnumeric_pid_is_malformed() {
        let m = format!("Sandbox: bash(notanumber) allow file-read-data /etc/hosts\n{RUN_ID}");
        assert!(matches!(
            parse_message(&m, RUN_ID),
            SeatbeltParseOutcome::Malformed(_)
        ));
    }

    #[test]
    fn empty_path_on_file_read_data_is_malformed() {
        let m = format!("Sandbox: bash(1) allow file-read-data\n{RUN_ID}");
        assert!(matches!(
            parse_message(&m, RUN_ID),
            SeatbeltParseOutcome::Malformed(_)
        ));
    }

    #[test]
    fn doc_comment_samples_all_parse() {
        let cases: &[(&str, SeatbeltParseOutcome)] = &[
            (
                "Sandbox: bash(3052) allow file-write-create /private/tmp/lockin-audit-q2-file3",
                SeatbeltParseOutcome::Event(InferEvent::Fs {
                    op: FsOp::Create,
                    path: PathBuf::from("/private/tmp/lockin-audit-q2-file3"),
                }),
            ),
            (
                "Sandbox: coreutils(3053) allow file-read-data /private/etc/hosts",
                SeatbeltParseOutcome::Event(InferEvent::Fs {
                    op: FsOp::Read,
                    path: PathBuf::from("/private/etc/hosts"),
                }),
            ),
            (
                "Sandbox: sandbox-exec(3052) allow process-exec* /bin/sh",
                SeatbeltParseOutcome::Event(InferEvent::Exec {
                    path: PathBuf::from("/bin/sh"),
                }),
            ),
            (
                "Sandbox: bash(3052) allow process-fork",
                SeatbeltParseOutcome::Skip,
            ),
        ];

        for (first, expected) in cases {
            let m = msg(first);
            assert_eq!(&parse_message(&m, RUN_ID), expected, "case: {first}");
        }

        let unsupported_cases = [
            "Sandbox: bash(3052) allow file-ioctl path:/dev/dtracehelper ioctl-command:(_IO \"h\" 4)",
            "Sandbox: bash(2901) allow sysctl-read kern.bootargs",
        ];
        for first in unsupported_cases {
            let m = msg(first);
            assert!(
                matches!(
                    parse_message(&m, RUN_ID),
                    SeatbeltParseOutcome::Unsupported(_)
                ),
                "case: {first}"
            );
        }
    }

    #[test]
    fn deny_action_still_classifies() {
        // We don't filter by action; under our profile it's always "allow",
        // but a "deny" should still classify rather than crash.
        assert!(matches!(
            parse_message(
                &msg("Sandbox: bash(1) deny file-read-data /etc/secret"),
                RUN_ID
            ),
            SeatbeltParseOutcome::Event(InferEvent::Fs { op: FsOp::Read, .. })
        ));
    }

    #[test]
    fn extra_whitespace_in_path_is_trimmed() {
        let m = msg("Sandbox: bash(1) allow file-read-data    /etc/hosts   ");
        assert_eq!(
            parse_message(&m, RUN_ID),
            SeatbeltParseOutcome::Event(InferEvent::Fs {
                op: FsOp::Read,
                path: p("/etc/hosts"),
            })
        );
    }
}
