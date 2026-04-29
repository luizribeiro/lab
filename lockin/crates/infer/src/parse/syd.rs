//! Parser for syd JSONL log records.

use std::path::PathBuf;

use crate::event::{DiagnosticLevel, FsOp, InferDiagnostic, InferEvent};

/// Outcome of parsing one syd JSONL line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SydParseOutcome {
    /// Successfully classified as an inference event.
    Event(InferEvent),
    /// Recognized as a syd record but irrelevant to inference (status,
    /// internal lifecycle, allow-without-violation noise, etc.). Skip.
    Skip,
    /// Recognized as describing a sandbox operation but not one we can
    /// translate into the lockin schema. Surfaces as a diagnostic; never
    /// becomes policy.
    Unsupported(InferDiagnostic),
    /// Line failed to parse as JSON or was malformed in a way we can't
    /// classify. Surfaces as a diagnostic at warn level.
    Malformed(InferDiagnostic),
}

const BACKEND: &str = "syd";

/// Parse one line of syd JSONL.
pub fn parse_line(line: &str) -> SydParseOutcome {
    let trimmed = line.trim_end_matches(['\n', '\r']);
    if trimmed.trim().is_empty() {
        return SydParseOutcome::Skip;
    }

    let value: serde_json::Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(e) => {
            return SydParseOutcome::Malformed(diag(
                DiagnosticLevel::Warn,
                format!("{BACKEND}: invalid JSON: {e}"),
            ));
        }
    };

    let obj = match value.as_object() {
        Some(o) => o,
        None => {
            return SydParseOutcome::Malformed(diag(
                DiagnosticLevel::Warn,
                format!("{BACKEND}: expected JSON object, got {trimmed}"),
            ));
        }
    };

    let event_kind = match obj.get("event").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return SydParseOutcome::Malformed(diag(
                DiagnosticLevel::Warn,
                format!("{BACKEND}: missing 'event' field: {trimmed}"),
            ));
        }
    };

    if event_kind != "violation" {
        return SydParseOutcome::Skip;
    }

    let operation = obj.get("operation").and_then(|v| v.as_str()).unwrap_or("");
    let path = obj.get("path").and_then(|v| v.as_str());

    match classify(operation) {
        OpClass::Read => fs_event(FsOp::Read, path, operation),
        OpClass::Stat => fs_event(FsOp::Stat, path, operation),
        OpClass::ReadDir => fs_event(FsOp::ReadDir, path, operation),
        OpClass::Write => fs_event(FsOp::Write, path, operation),
        OpClass::Create => fs_event(FsOp::Create, path, operation),
        OpClass::Delete => fs_event(FsOp::Delete, path, operation),
        OpClass::Exec => match path {
            Some(p) => SydParseOutcome::Event(InferEvent::Exec {
                path: PathBuf::from(p),
            }),
            None => SydParseOutcome::Malformed(diag(
                DiagnosticLevel::Warn,
                format!("{BACKEND}: exec record without 'path': {trimmed}"),
            )),
        },
        OpClass::Unknown => {
            let detail = match path {
                Some(p) => format!(" (path: {p})"),
                None => String::new(),
            };
            SydParseOutcome::Unsupported(diag(
                DiagnosticLevel::Warn,
                format!(
                    "{BACKEND}: operation {:?} has no lockin schema mapping{detail}",
                    operation
                ),
            ))
        }
    }
}

fn fs_event(op: FsOp, path: Option<&str>, operation: &str) -> SydParseOutcome {
    match path {
        Some(p) => SydParseOutcome::Event(InferEvent::Fs {
            op,
            path: PathBuf::from(p),
        }),
        None => SydParseOutcome::Malformed(diag(
            DiagnosticLevel::Warn,
            format!("{BACKEND}: {operation} record without 'path'"),
        )),
    }
}

fn diag(level: DiagnosticLevel, message: String) -> InferDiagnostic {
    InferDiagnostic { level, message }
}

enum OpClass {
    Read,
    Stat,
    ReadDir,
    Write,
    Create,
    Delete,
    Exec,
    Unknown,
}

fn classify(op: &str) -> OpClass {
    match op {
        "read" | "open" | "openat" | "readlink" => OpClass::Read,
        "stat" | "lstat" | "fstatat" | "access" | "faccessat" => OpClass::Stat,
        "readdir" | "getdents" | "getdents64" => OpClass::ReadDir,
        "write" => OpClass::Write,
        "create" | "truncate" | "creat" => OpClass::Create,
        "unlink" | "rmdir" | "unlinkat" | "delete" => OpClass::Delete,
        "exec" | "execve" | "execveat" => OpClass::Exec,
        _ => OpClass::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn line(op: &str, path: &str) -> String {
        format!(
            r#"{{"level":"notice","event":"violation","action":"allow","operation":"{op}","path":"{path}","pid":1234}}"#
        )
    }

    #[test]
    fn read_op_maps_to_fs_read() {
        assert_eq!(
            parse_line(&line("read", "/etc/hosts")),
            SydParseOutcome::Event(InferEvent::Fs {
                op: FsOp::Read,
                path: p("/etc/hosts"),
            })
        );
    }

    #[test]
    fn open_and_openat_map_to_fs_read() {
        for op in ["open", "openat"] {
            let outcome = parse_line(&line(op, "/etc/hosts"));
            assert_eq!(
                outcome,
                SydParseOutcome::Event(InferEvent::Fs {
                    op: FsOp::Read,
                    path: p("/etc/hosts"),
                }),
                "operation {op}"
            );
        }
    }

    #[test]
    fn readlink_maps_to_read() {
        assert!(matches!(
            parse_line(&line("readlink", "/proc/self/exe")),
            SydParseOutcome::Event(InferEvent::Fs { op: FsOp::Read, .. })
        ));
    }

    #[test]
    fn stat_family_maps_to_fs_stat() {
        for op in ["stat", "lstat", "fstatat", "access", "faccessat"] {
            assert!(
                matches!(
                    parse_line(&line(op, "/etc/hosts")),
                    SydParseOutcome::Event(InferEvent::Fs { op: FsOp::Stat, .. })
                ),
                "operation {op}"
            );
        }
    }

    #[test]
    fn readdir_family_maps_to_fs_readdir() {
        for op in ["readdir", "getdents", "getdents64"] {
            assert!(
                matches!(
                    parse_line(&line(op, "/etc")),
                    SydParseOutcome::Event(InferEvent::Fs {
                        op: FsOp::ReadDir,
                        ..
                    })
                ),
                "operation {op}"
            );
        }
    }

    #[test]
    fn write_maps_to_fs_write() {
        assert_eq!(
            parse_line(&line("write", "/tmp/work/out.txt")),
            SydParseOutcome::Event(InferEvent::Fs {
                op: FsOp::Write,
                path: p("/tmp/work/out.txt"),
            })
        );
    }

    #[test]
    fn create_family_maps_to_fs_create() {
        for op in ["create", "truncate", "creat"] {
            assert!(
                matches!(
                    parse_line(&line(op, "/tmp/work/out.txt")),
                    SydParseOutcome::Event(InferEvent::Fs {
                        op: FsOp::Create,
                        ..
                    })
                ),
                "operation {op}"
            );
        }
    }

    #[test]
    fn delete_family_maps_to_fs_delete() {
        for op in ["unlink", "rmdir", "unlinkat", "delete"] {
            assert!(
                matches!(
                    parse_line(&line(op, "/tmp/work/old.txt")),
                    SydParseOutcome::Event(InferEvent::Fs {
                        op: FsOp::Delete,
                        ..
                    })
                ),
                "operation {op}"
            );
        }
    }

    #[test]
    fn exec_family_maps_to_exec_event() {
        for op in ["exec", "execve", "execveat"] {
            assert_eq!(
                parse_line(&line(op, "/usr/bin/ls")),
                SydParseOutcome::Event(InferEvent::Exec {
                    path: p("/usr/bin/ls"),
                }),
                "operation {op}"
            );
        }
    }

    #[test]
    fn unknown_operation_is_unsupported_with_op_name() {
        let outcome = parse_line(&line("ioctl", "/dev/null"));
        let SydParseOutcome::Unsupported(d) = outcome else {
            panic!("expected Unsupported, got {outcome:?}");
        };
        assert!(d.message.contains("ioctl"), "message: {}", d.message);
        assert_eq!(d.level, DiagnosticLevel::Warn);
    }

    #[test]
    fn non_violation_event_is_skipped() {
        let line = r#"{"event":"status","ts":"now"}"#;
        assert_eq!(parse_line(line), SydParseOutcome::Skip);
    }

    #[test]
    fn missing_event_field_is_malformed() {
        let line = r#"{"operation":"read","path":"/etc/hosts"}"#;
        assert!(matches!(parse_line(line), SydParseOutcome::Malformed(_)));
    }

    #[test]
    fn missing_path_for_fs_op_is_malformed() {
        let line = r#"{"event":"violation","operation":"read"}"#;
        assert!(matches!(parse_line(line), SydParseOutcome::Malformed(_)));
    }

    #[test]
    fn missing_path_for_exec_is_malformed() {
        let line = r#"{"event":"violation","operation":"execve"}"#;
        assert!(matches!(parse_line(line), SydParseOutcome::Malformed(_)));
    }

    #[test]
    fn malformed_json_is_malformed() {
        let outcome = parse_line("{not json");
        let SydParseOutcome::Malformed(d) = outcome else {
            panic!("expected Malformed, got {outcome:?}");
        };
        assert!(d.message.contains("invalid JSON"), "{}", d.message);
    }

    #[test]
    fn empty_line_is_skipped() {
        assert_eq!(parse_line(""), SydParseOutcome::Skip);
    }

    #[test]
    fn whitespace_only_line_is_skipped() {
        assert_eq!(parse_line("   \t  \n"), SydParseOutcome::Skip);
    }

    #[test]
    fn unknown_extra_fields_are_tolerated() {
        let line = r#"{"event":"violation","operation":"openat","path":"/etc/hosts","pid":1234,"future_field":{"nested":true},"another":42}"#;
        assert_eq!(
            parse_line(line),
            SydParseOutcome::Event(InferEvent::Fs {
                op: FsOp::Read,
                path: p("/etc/hosts"),
            })
        );
    }

    #[test]
    fn doc_comment_sample_lines_all_parse() {
        let samples = [
            (
                r#"{"level":"notice","ts":"2025-04-29T12:00:00Z","event":"violation","action":"allow","syscall":"openat","operation":"read","path":"/etc/hosts","pid":1234}"#,
                InferEvent::Fs {
                    op: FsOp::Read,
                    path: p("/etc/hosts"),
                },
            ),
            (
                r#"{"level":"notice","event":"violation","action":"allow","operation":"write","path":"/tmp/work/out.txt","syscall":"openat","pid":1234}"#,
                InferEvent::Fs {
                    op: FsOp::Write,
                    path: p("/tmp/work/out.txt"),
                },
            ),
            (
                r#"{"level":"notice","event":"violation","action":"allow","operation":"create","path":"/tmp/work/out.txt","syscall":"openat","pid":1234}"#,
                InferEvent::Fs {
                    op: FsOp::Create,
                    path: p("/tmp/work/out.txt"),
                },
            ),
            (
                r#"{"level":"notice","event":"violation","action":"allow","operation":"unlink","path":"/tmp/work/old.txt","pid":1234}"#,
                InferEvent::Fs {
                    op: FsOp::Delete,
                    path: p("/tmp/work/old.txt"),
                },
            ),
            (
                r#"{"level":"notice","event":"violation","action":"allow","operation":"stat","path":"/usr/lib/x86_64-linux-gnu/libc.so.6","pid":1234}"#,
                InferEvent::Fs {
                    op: FsOp::Stat,
                    path: p("/usr/lib/x86_64-linux-gnu/libc.so.6"),
                },
            ),
            (
                r#"{"level":"notice","event":"violation","action":"allow","operation":"readdir","path":"/etc","pid":1234}"#,
                InferEvent::Fs {
                    op: FsOp::ReadDir,
                    path: p("/etc"),
                },
            ),
            (
                r#"{"level":"notice","event":"violation","action":"allow","operation":"exec","path":"/usr/bin/ls","pid":1234}"#,
                InferEvent::Exec {
                    path: p("/usr/bin/ls"),
                },
            ),
        ];

        for (raw, expected) in samples {
            assert_eq!(
                parse_line(raw),
                SydParseOutcome::Event(expected),
                "line: {raw}"
            );
        }
    }

    #[test]
    fn trailing_newline_is_stripped() {
        let line = format!("{}\n", line("read", "/etc/hosts"));
        assert!(matches!(
            parse_line(&line),
            SydParseOutcome::Event(InferEvent::Fs { op: FsOp::Read, .. })
        ));
    }
}
