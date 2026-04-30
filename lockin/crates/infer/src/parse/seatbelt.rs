//! Parser for macOS Seatbelt sandbox report messages.

use std::path::PathBuf;

use crate::event::{AccessAction, AccessEvent, DiagnosticLevel, FsOp, InferDiagnostic, InferEvent};

/// Outcome of parsing one Seatbelt eventMessage string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SeatbeltParseOutcome {
    /// Successfully classified as an inference event tagged with our
    /// RUN_ID and the sandbox's decision (allow/deny). Callers filter
    /// by action.
    Event(AccessEvent),
    /// Recognized as a Seatbelt report but not relevant (different RUN_ID,
    /// duplicate-compression line, allow process-fork without a path,
    /// etc.). Skip silently.
    Skip,
    /// Recognized as a Seatbelt operation but not translatable into the
    /// concrete `InferEvent` schema (mach-lookup, sysctl-read,
    /// file-ioctl, network*, etc.). Carries both an `AccessEvent`
    /// wrapping `InferEvent::Unsupported` (so deny-trace callers can
    /// still report the access in their denial log) and a human-readable
    /// diagnostic (so infer callers can surface the warning).
    Unsupported {
        event: AccessEvent,
        diagnostic: InferDiagnostic,
    },
    /// Line did not match the expected Seatbelt grammar.
    Malformed(InferDiagnostic),
}

const BACKEND: &str = "seatbelt";

/// Parse one Seatbelt eventMessage. `expected_run_id` is the per-run UUID
/// the caller embedded into the sandbox profile via
/// `(with message (param "RUN_ID"))`.
/// Messages tagged with a different (or missing) RUN_ID return `Skip`.
pub fn parse_access_message(message: &str, expected_run_id: &str) -> SeatbeltParseOutcome {
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
    let action_tok = tokens.next().unwrap_or("");
    let operation = tokens.next().unwrap_or("");
    let remainder = tokens.next().unwrap_or("").trim();

    if action_tok.is_empty() || operation.is_empty() {
        return malformed(format!("missing action/operation: {first}"));
    }

    // Seatbelt emits the action token as a bare word for user-space
    // `with report` allow/deny events, but kernel-side `Sandbox.kext`
    // tags its auto-published deny lines with a count suffix —
    // `deny(1)`, `deny(2)`, ... — so strip any `(N)` before matching.
    let action_base = action_tok
        .split_once('(')
        .map(|(b, _)| b)
        .unwrap_or(action_tok);
    let action = match action_base {
        "allow" => AccessAction::Allow,
        "deny" => AccessAction::Deny,
        // Seatbelt has no formal "warn" mode; surface anything else as
        // Warn defensively rather than dropping the event.
        _ => AccessAction::Warn,
    };

    classify(action, operation, remainder, first)
}

fn is_duplicate_report(s: &str) -> bool {
    let mut iter = s.splitn(2, ' ');
    let first = iter.next().unwrap_or("");
    let after = iter.next().unwrap_or("");
    first.parse::<u64>().is_ok()
        && (after.starts_with("duplicate report for ")
            || after.starts_with("duplicate reports for "))
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

fn classify(
    action: AccessAction,
    op: &str,
    remainder: &str,
    first_line: &str,
) -> SeatbeltParseOutcome {
    let path_op = |fs_op: FsOp| -> SeatbeltParseOutcome {
        if remainder.is_empty() {
            return malformed(format!("{op}: missing path in {first_line}"));
        }
        SeatbeltParseOutcome::Event(AccessEvent {
            action,
            event: InferEvent::Fs {
                op: fs_op,
                path: PathBuf::from(remainder),
            },
        })
    };

    match op {
        "file-read-data" => path_op(FsOp::Read),
        "file-read-metadata" => path_op(FsOp::Stat),
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
            SeatbeltParseOutcome::Event(AccessEvent {
                action,
                event: InferEvent::Exec {
                    path: PathBuf::from(remainder),
                },
            })
        }
        "process-fork" => SeatbeltParseOutcome::Skip,
        _ => {
            let reason = format!("operation {op:?} has no lockin schema mapping");
            let raw = if remainder.is_empty() {
                op.to_string()
            } else {
                format!("{op} {remainder}")
            };
            SeatbeltParseOutcome::Unsupported {
                event: AccessEvent {
                    action,
                    event: InferEvent::Unsupported {
                        backend: BACKEND,
                        raw,
                        reason: reason.clone(),
                    },
                },
                diagnostic: diag(
                    DiagnosticLevel::Warn,
                    format!("{BACKEND}: {reason} ({first_line})"),
                ),
            }
        }
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

    fn fs(action: AccessAction, op: FsOp, path: &str) -> SeatbeltParseOutcome {
        SeatbeltParseOutcome::Event(AccessEvent {
            action,
            event: InferEvent::Fs { op, path: p(path) },
        })
    }

    fn exec(action: AccessAction, path: &str) -> SeatbeltParseOutcome {
        SeatbeltParseOutcome::Event(AccessEvent {
            action,
            event: InferEvent::Exec { path: p(path) },
        })
    }

    #[test]
    fn file_read_data_maps_to_read() {
        assert_eq!(
            parse_access_message(
                &msg("Sandbox: coreutils(3053) allow file-read-data /private/etc/hosts"),
                RUN_ID
            ),
            fs(AccessAction::Allow, FsOp::Read, "/private/etc/hosts")
        );
    }

    #[test]
    fn file_read_metadata_maps_to_stat() {
        assert!(matches!(
            parse_access_message(
                &msg("Sandbox: bash(1) allow file-read-metadata /etc/hosts"),
                RUN_ID
            ),
            SeatbeltParseOutcome::Event(AccessEvent {
                event: InferEvent::Fs { op: FsOp::Stat, .. },
                ..
            })
        ));
    }

    #[test]
    fn file_readdir_maps_to_readdir() {
        assert!(matches!(
            parse_access_message(&msg("Sandbox: ls(1) allow file-readdir /etc"), RUN_ID),
            SeatbeltParseOutcome::Event(AccessEvent {
                event: InferEvent::Fs {
                    op: FsOp::ReadDir,
                    ..
                },
                ..
            })
        ));
    }

    #[test]
    fn file_write_create_maps_to_create() {
        assert_eq!(
            parse_access_message(
                &msg("Sandbox: bash(3052) allow file-write-create /private/tmp/lockin-audit-q2-file3"),
                RUN_ID
            ),
            fs(
                AccessAction::Allow,
                FsOp::Create,
                "/private/tmp/lockin-audit-q2-file3"
            )
        );
    }

    #[test]
    fn file_write_data_maps_to_write() {
        assert!(matches!(
            parse_access_message(
                &msg("Sandbox: bash(1) allow file-write-data /tmp/out"),
                RUN_ID
            ),
            SeatbeltParseOutcome::Event(AccessEvent {
                event: InferEvent::Fs {
                    op: FsOp::Write,
                    ..
                },
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
                    parse_access_message(&msg, RUN_ID),
                    SeatbeltParseOutcome::Event(AccessEvent {
                        event: InferEvent::Fs {
                            op: FsOp::Write,
                            ..
                        },
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
            parse_access_message(
                &msg("Sandbox: bash(1) allow file-write-unlink /tmp/old"),
                RUN_ID
            ),
            SeatbeltParseOutcome::Event(AccessEvent {
                event: InferEvent::Fs {
                    op: FsOp::Delete,
                    ..
                },
                ..
            })
        ));
    }

    #[test]
    fn process_exec_star_maps_to_exec() {
        assert_eq!(
            parse_access_message(
                &msg("Sandbox: sandbox-exec(3052) allow process-exec* /bin/sh"),
                RUN_ID
            ),
            exec(AccessAction::Allow, "/bin/sh")
        );
    }

    #[test]
    fn process_exec_interpreter_maps_to_exec() {
        assert!(matches!(
            parse_access_message(
                &msg("Sandbox: sh(1) allow process-exec-interpreter /usr/bin/perl"),
                RUN_ID
            ),
            SeatbeltParseOutcome::Event(AccessEvent {
                event: InferEvent::Exec { .. },
                ..
            })
        ));
    }

    #[test]
    fn seatbelt_allow_classifies_as_allow() {
        let outcome = parse_access_message(
            &msg("Sandbox: bash(1) allow file-read-data /etc/hosts"),
            RUN_ID,
        );
        assert_eq!(outcome, fs(AccessAction::Allow, FsOp::Read, "/etc/hosts"));
    }

    #[test]
    fn seatbelt_deny_classifies_as_deny() {
        let outcome = parse_access_message(
            &msg("Sandbox: probe(123) deny file-read-data /etc/secret"),
            RUN_ID,
        );
        assert_eq!(outcome, fs(AccessAction::Deny, FsOp::Read, "/etc/secret"));
    }

    #[test]
    fn seatbelt_deny_with_count_suffix_classifies_as_deny() {
        // Kernel-emitted Sandbox lines (from `(deny default)` matches,
        // not user-space `with report`) include a `deny(N)` count.
        let outcome = parse_access_message(
            &msg("Sandbox: probe(123) deny(1) file-read-data /etc/secret"),
            RUN_ID,
        );
        assert_eq!(outcome, fs(AccessAction::Deny, FsOp::Read, "/etc/secret"));
    }

    #[test]
    fn process_fork_is_skip() {
        assert_eq!(
            parse_access_message(&msg("Sandbox: bash(3052) allow process-fork"), RUN_ID),
            SeatbeltParseOutcome::Skip
        );
    }

    #[test]
    fn mach_lookup_is_unsupported() {
        let outcome = parse_access_message(
            &msg("Sandbox: bash(1) allow mach-lookup com.apple.foo"),
            RUN_ID,
        );
        let SeatbeltParseOutcome::Unsupported { diagnostic, .. } = outcome else {
            panic!("expected Unsupported, got {outcome:?}");
        };
        assert!(diagnostic.message.contains("mach-lookup"));
    }

    #[test]
    fn sysctl_read_is_unsupported() {
        let outcome = parse_access_message(
            &msg("Sandbox: bash(2901) allow sysctl-read kern.bootargs"),
            RUN_ID,
        );
        let SeatbeltParseOutcome::Unsupported { diagnostic, .. } = outcome else {
            panic!("expected Unsupported, got {outcome:?}");
        };
        assert!(diagnostic.message.contains("sysctl-read"));
    }

    #[test]
    fn network_outbound_is_unsupported() {
        let outcome = parse_access_message(
            &msg("Sandbox: bash(1) allow network-outbound remote:*:443"),
            RUN_ID,
        );
        let SeatbeltParseOutcome::Unsupported { diagnostic, .. } = outcome else {
            panic!("expected Unsupported, got {outcome:?}");
        };
        assert!(diagnostic.message.contains("network-outbound"));
    }

    #[test]
    fn file_ioctl_is_unsupported() {
        let outcome = parse_access_message(
            &msg(
                "Sandbox: bash(3052) allow file-ioctl path:/dev/dtracehelper ioctl-command:(_IO \"h\" 4)",
            ),
            RUN_ID,
        );
        assert!(matches!(outcome, SeatbeltParseOutcome::Unsupported { .. }));
    }

    #[test]
    fn different_run_id_is_skip() {
        let other = "Sandbox: bash(1) allow file-read-data /etc/hosts\nlockin-run-OTHER-RUN-ID";
        assert_eq!(
            parse_access_message(other, RUN_ID),
            SeatbeltParseOutcome::Skip
        );
    }

    #[test]
    fn missing_run_id_line_is_skip() {
        let only = "Sandbox: bash(1) allow file-read-data /etc/hosts";
        assert_eq!(
            parse_access_message(only, RUN_ID),
            SeatbeltParseOutcome::Skip
        );
    }

    #[test]
    fn duplicate_report_line_is_skip() {
        let dup = "1 duplicate report for Sandbox: bash(2901) allow file-read-data /bin/bash";
        assert_eq!(
            parse_access_message(dup, RUN_ID),
            SeatbeltParseOutcome::Skip
        );
    }

    #[test]
    fn double_digit_duplicate_report_line_is_skip() {
        let dup = "37 duplicate report for Sandbox: bash(2901) allow file-read-data /bin/bash";
        assert_eq!(
            parse_access_message(dup, RUN_ID),
            SeatbeltParseOutcome::Skip
        );
    }

    #[test]
    fn unsupported_op_carries_event_with_action_and_raw_remainder() {
        let outcome = parse_access_message(
            &msg("Sandbox: claude(81767) deny(1) network-outbound /private/var/run/mDNSResponder"),
            RUN_ID,
        );
        let SeatbeltParseOutcome::Unsupported { event, .. } = outcome else {
            panic!("expected Unsupported, got {outcome:?}");
        };
        assert_eq!(event.action, AccessAction::Deny);
        let InferEvent::Unsupported { backend, raw, .. } = event.event else {
            panic!("expected InferEvent::Unsupported, got {:?}", event.event);
        };
        assert_eq!(backend, "seatbelt");
        assert_eq!(raw, "network-outbound /private/var/run/mDNSResponder");
    }

    #[test]
    fn plural_duplicate_reports_line_is_skip() {
        // macOS pluralizes the count word when N > 1: `3 duplicate reports for ...`.
        let dup = "3 duplicate reports for Sandbox: bash(2901) allow file-read-data /bin/bash";
        assert_eq!(
            parse_access_message(dup, RUN_ID),
            SeatbeltParseOutcome::Skip
        );
    }

    #[test]
    fn empty_message_is_skip() {
        assert_eq!(parse_access_message("", RUN_ID), SeatbeltParseOutcome::Skip);
        assert_eq!(
            parse_access_message("   \n\n", RUN_ID),
            SeatbeltParseOutcome::Skip
        );
    }

    #[test]
    fn missing_sandbox_prefix_is_malformed() {
        let m = format!("notasandboxline\n{RUN_ID}");
        assert!(matches!(
            parse_access_message(&m, RUN_ID),
            SeatbeltParseOutcome::Malformed(_)
        ));
    }

    #[test]
    fn missing_pid_parens_is_malformed() {
        let m = format!("Sandbox: bash allow file-read-data /etc/hosts\n{RUN_ID}");
        assert!(matches!(
            parse_access_message(&m, RUN_ID),
            SeatbeltParseOutcome::Malformed(_)
        ));
    }

    #[test]
    fn nonnumeric_pid_is_malformed() {
        let m = format!("Sandbox: bash(notanumber) allow file-read-data /etc/hosts\n{RUN_ID}");
        assert!(matches!(
            parse_access_message(&m, RUN_ID),
            SeatbeltParseOutcome::Malformed(_)
        ));
    }

    #[test]
    fn empty_path_on_file_read_data_is_malformed() {
        let m = format!("Sandbox: bash(1) allow file-read-data\n{RUN_ID}");
        assert!(matches!(
            parse_access_message(&m, RUN_ID),
            SeatbeltParseOutcome::Malformed(_)
        ));
    }

    #[test]
    fn doc_comment_samples_all_parse() {
        let cases: &[(&str, SeatbeltParseOutcome)] = &[
            (
                "Sandbox: bash(3052) allow file-write-create /private/tmp/lockin-audit-q2-file3",
                fs(
                    AccessAction::Allow,
                    FsOp::Create,
                    "/private/tmp/lockin-audit-q2-file3",
                ),
            ),
            (
                "Sandbox: coreutils(3053) allow file-read-data /private/etc/hosts",
                fs(AccessAction::Allow, FsOp::Read, "/private/etc/hosts"),
            ),
            (
                "Sandbox: sandbox-exec(3052) allow process-exec* /bin/sh",
                exec(AccessAction::Allow, "/bin/sh"),
            ),
            (
                "Sandbox: bash(3052) allow process-fork",
                SeatbeltParseOutcome::Skip,
            ),
        ];

        for (first, expected) in cases {
            let m = msg(first);
            assert_eq!(&parse_access_message(&m, RUN_ID), expected, "case: {first}");
        }

        let unsupported_cases = [
            "Sandbox: bash(3052) allow file-ioctl path:/dev/dtracehelper ioctl-command:(_IO \"h\" 4)",
            "Sandbox: bash(2901) allow sysctl-read kern.bootargs",
        ];
        for first in unsupported_cases {
            let m = msg(first);
            assert!(
                matches!(
                    parse_access_message(&m, RUN_ID),
                    SeatbeltParseOutcome::Unsupported { .. }
                ),
                "case: {first}"
            );
        }
    }

    #[test]
    fn extra_whitespace_in_path_is_trimmed() {
        let m = msg("Sandbox: bash(1) allow file-read-data    /etc/hosts   ");
        assert_eq!(
            parse_access_message(&m, RUN_ID),
            fs(AccessAction::Allow, FsOp::Read, "/etc/hosts")
        );
    }
}
