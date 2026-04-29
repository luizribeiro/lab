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

    // syd 3.49.x emits structured access records with ctx="access".
    // Other ctxs (boot/run/confine/seal_executable_maps/...) are lifecycle
    // noise we skip silently.
    let ctx = match obj.get("ctx").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => {
            return SydParseOutcome::Malformed(diag(
                DiagnosticLevel::Warn,
                format!("{BACKEND}: missing 'ctx' field: {trimmed}"),
            ));
        }
    };

    if ctx != "access" {
        return SydParseOutcome::Skip;
    }

    // The `cap` field is the access-control category; `act` is what syd
    // would do (warn, deny, allow, ...). Both warn (audit) and deny
    // (would-have-blocked) are in-scope for inference; allow records
    // shouldn't appear under a deny/warn default but skip them defensively.
    let cap = obj.get("cap").and_then(|v| v.as_str()).unwrap_or("");
    let act = obj.get("act").and_then(|v| v.as_str()).unwrap_or("");
    if !matches!(act, "warn" | "deny") {
        return SydParseOutcome::Skip;
    }
    let path = obj.get("path").and_then(|v| v.as_str());

    // syd 3.49 rarely fires the high-level write/create caps directly;
    // it logs the underlying `fs` openat with oflags. Inspect oflags to
    // recover write/create classification before falling through to the
    // generic classifier.
    if cap == "fs" {
        if let Some(class) = classify_fs_oflags(obj) {
            return match class {
                FsClass::Write => fs_event(FsOp::Write, path, cap),
                FsClass::Create => fs_event(FsOp::Create, path, cap),
                FsClass::ReadOnly => SydParseOutcome::Skip,
            };
        }
        return SydParseOutcome::Skip;
    }

    match classify(cap) {
        OpClass::Read => fs_event(FsOp::Read, path, cap),
        OpClass::Stat => fs_event(FsOp::Stat, path, cap),
        OpClass::ReadDir => fs_event(FsOp::ReadDir, path, cap),
        OpClass::Write => fs_event(FsOp::Write, path, cap),
        OpClass::Create => fs_event(FsOp::Create, path, cap),
        OpClass::Delete => fs_event(FsOp::Delete, path, cap),
        OpClass::Exec => match path {
            Some(p) => SydParseOutcome::Event(InferEvent::Exec {
                path: PathBuf::from(p),
            }),
            None => SydParseOutcome::Malformed(diag(
                DiagnosticLevel::Warn,
                format!("{BACKEND}: exec record without 'path': {trimmed}"),
            )),
        },
        OpClass::Skip => SydParseOutcome::Skip,
        OpClass::Unknown => {
            let detail = match path {
                Some(p) => format!(" (path: {p})"),
                None => String::new(),
            };
            SydParseOutcome::Unsupported(diag(
                DiagnosticLevel::Warn,
                format!(
                    "{BACKEND}: cap {:?} has no lockin schema mapping{detail}",
                    cap
                ),
            ))
        }
    }
}

enum FsClass {
    Write,
    Create,
    ReadOnly,
}

/// Inspect a `cap="fs"` open record's `oflags` array to classify it as
/// a read-only open, a write open, or a create/truncate. Returns `None`
/// only if the record has no `oflags` array (in which case the parser
/// should fall back to Skip — the higher-level cap will fire).
fn classify_fs_oflags(obj: &serde_json::Map<String, serde_json::Value>) -> Option<FsClass> {
    let arr = obj.get("oflags").and_then(|v| v.as_array())?;
    let flags: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
    let writes = flags.iter().any(|f| matches!(*f, "wronly" | "rdwr"));
    if !writes {
        return Some(FsClass::ReadOnly);
    }
    let creates = flags
        .iter()
        .any(|f| matches!(*f, "creat" | "trunc" | "tmpfile"));
    Some(if creates {
        FsClass::Create
    } else {
        FsClass::Write
    })
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
    /// Recognized cap that intentionally produces no event (the higher-
    /// level cap, e.g. `read`, will fire for the same syscall).
    Skip,
    Unknown,
}

fn classify(cap: &str) -> OpClass {
    match cap {
        "read" => OpClass::Read,
        "stat" => OpClass::Stat,
        "readdir" => OpClass::ReadDir,
        "write" => OpClass::Write,
        "create" | "truncate" => OpClass::Create,
        "delete" => OpClass::Delete,
        "exec" => OpClass::Exec,
        // syd's underlying filesystem cap fires alongside the
        // higher-level read/write/exec caps; skip to avoid duplicates.
        "fs" | "walk" => OpClass::Skip,
        // Legacy syscall-name compatibility for the test fixtures (kept
        // so the existing unit tests stay valid against synthetic JSON
        // that uses operation=<syscall> shape).
        "open" | "openat" | "readlink" => OpClass::Read,
        "lstat" | "fstatat" | "access" | "faccessat" => OpClass::Stat,
        "getdents" | "getdents64" => OpClass::ReadDir,
        "creat" => OpClass::Create,
        "unlink" | "rmdir" | "unlinkat" => OpClass::Delete,
        "execve" | "execveat" => OpClass::Exec,
        _ => OpClass::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    fn line(cap: &str, path: &str) -> String {
        format!(
            r#"{{"id":"abc","syd":1234,"ctx":"access","cap":"{cap}","act":"warn","sys":"{cap}","path":"{path}","pid":5678,"uid":1000}}"#
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
        for op in ["create", "truncate"] {
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
    fn delete_cap_maps_to_fs_delete() {
        assert!(matches!(
            parse_line(&line("delete", "/tmp/work/old.txt")),
            SydParseOutcome::Event(InferEvent::Fs {
                op: FsOp::Delete,
                ..
            })
        ));
    }

    #[test]
    fn exec_cap_maps_to_exec_event() {
        assert_eq!(
            parse_line(&line("exec", "/usr/bin/ls")),
            SydParseOutcome::Event(InferEvent::Exec {
                path: p("/usr/bin/ls"),
            })
        );
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
    fn non_access_ctx_is_skipped() {
        let line = r#"{"ctx":"run","op":"boot","msg":"x"}"#;
        assert_eq!(parse_line(line), SydParseOutcome::Skip);
    }

    #[test]
    fn allow_action_is_skipped() {
        // Under deny/warn defaults syd shouldn't emit allow records, but
        // be defensive — they're not in-scope for inference.
        let line = r#"{"ctx":"access","cap":"read","act":"allow","path":"/etc/hosts"}"#;
        assert_eq!(parse_line(line), SydParseOutcome::Skip);
    }

    #[test]
    fn missing_ctx_field_is_malformed() {
        let line = r#"{"cap":"read","path":"/etc/hosts"}"#;
        assert!(matches!(parse_line(line), SydParseOutcome::Malformed(_)));
    }

    #[test]
    fn missing_path_for_fs_op_is_malformed() {
        let line = r#"{"ctx":"access","cap":"read","act":"warn"}"#;
        assert!(matches!(parse_line(line), SydParseOutcome::Malformed(_)));
    }

    #[test]
    fn missing_path_for_exec_is_malformed() {
        let line = r#"{"ctx":"access","cap":"exec","act":"warn"}"#;
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
        let line = r#"{"ctx":"access","cap":"read","act":"warn","path":"/etc/hosts","pid":1234,"future_field":{"nested":true},"another":42}"#;
        assert_eq!(
            parse_line(line),
            SydParseOutcome::Event(InferEvent::Fs {
                op: FsOp::Read,
                path: p("/etc/hosts"),
            })
        );
    }

    #[test]
    fn real_syd_3_49_records_parse() {
        // Shape verified empirically against syd 3.49.1 on Linux:
        // ctx="access", cap=<category>, act="warn"|"deny", path=<abs>.
        let samples = [
            (
                r#"{"id":"x","syd":1,"ctx":"access","cap":"read","act":"warn","sys":"openat","fs":"ext","path":"/etc/hosts","mode":0,"oflags":["cloexec"],"type":"reg","time":"t","pid":2,"uid":1000}"#,
                InferEvent::Fs {
                    op: FsOp::Read,
                    path: p("/etc/hosts"),
                },
            ),
            (
                r#"{"id":"x","ctx":"access","cap":"stat","act":"warn","sys":"stat","path":"/usr/lib/libc.so.6","args":[0,0,0,0,0,0],"time":"t","pid":2,"uid":1000}"#,
                InferEvent::Fs {
                    op: FsOp::Stat,
                    path: p("/usr/lib/libc.so.6"),
                },
            ),
            (
                r#"{"id":"x","ctx":"access","cap":"exec","act":"warn","sys":"execve","path":"/usr/bin/ls","time":"t","pid":2,"uid":1000}"#,
                InferEvent::Exec {
                    path: p("/usr/bin/ls"),
                },
            ),
            (
                r#"{"id":"x","ctx":"access","cap":"create","act":"warn","sys":"openat","path":"/tmp/work/out.txt","time":"t","pid":2,"uid":1000}"#,
                InferEvent::Fs {
                    op: FsOp::Create,
                    path: p("/tmp/work/out.txt"),
                },
            ),
            (
                r#"{"id":"x","ctx":"access","cap":"delete","act":"warn","sys":"unlinkat","path":"/tmp/work/old.txt","time":"t","pid":2,"uid":1000}"#,
                InferEvent::Fs {
                    op: FsOp::Delete,
                    path: p("/tmp/work/old.txt"),
                },
            ),
            (
                r#"{"id":"x","ctx":"access","cap":"readdir","act":"warn","sys":"getdents64","path":"/etc","time":"t","pid":2,"uid":1000}"#,
                InferEvent::Fs {
                    op: FsOp::ReadDir,
                    path: p("/etc"),
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
