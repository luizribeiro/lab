//! Parser for syd JSONL log records.

use std::path::PathBuf;

use crate::event::{AccessAction, AccessEvent, DiagnosticLevel, FsOp, InferDiagnostic, InferEvent};

/// Outcome of parsing one syd JSONL line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SydParseOutcome {
    /// Successfully classified as an inference event tagged with the
    /// sandbox action (allow/warn/deny). Callers filter by action.
    Event(AccessEvent),
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
pub fn parse_access_line(line: &str) -> SydParseOutcome {
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

    // `cap` is the access-control category; `act` is what syd did
    // (allow/warn/deny). All three are in-scope: callers filter as
    // appropriate (inference wants warn|deny, trace wants deny only).
    //
    // `cap` is sometimes a string (e.g. `"read"`) and sometimes an
    // array (e.g. `["write","create"]`); normalize to a single best-
    // match category (Create wins over Write since it's more specific).
    let cap = normalize_cap(obj.get("cap"));
    let act = obj.get("act").and_then(|v| v.as_str()).unwrap_or("");
    let action = match act {
        "allow" => AccessAction::Allow,
        "warn" => AccessAction::Warn,
        "deny" => AccessAction::Deny,
        _ => return SydParseOutcome::Skip,
    };
    let path = obj.get("path").and_then(|v| v.as_str());

    // syd 3.49 rarely fires the high-level write/create caps directly;
    // it logs the underlying `fs` openat with oflags. Inspect oflags to
    // recover write/create classification before falling through to the
    // generic classifier.
    if cap == "fs" {
        if let Some(class) = classify_fs_oflags(obj) {
            return match class {
                FsClass::Write => fs_event(action, FsOp::Write, path, &cap),
                FsClass::Create => fs_event(action, FsOp::Create, path, &cap),
                FsClass::ReadOnly => SydParseOutcome::Skip,
            };
        }
        return SydParseOutcome::Skip;
    }

    match classify(&cap) {
        OpClass::Read => fs_event(action, FsOp::Read, path, &cap),
        OpClass::Stat => fs_event(action, FsOp::Stat, path, &cap),
        OpClass::ReadDir => fs_event(action, FsOp::ReadDir, path, &cap),
        OpClass::Write => fs_event(action, FsOp::Write, path, &cap),
        OpClass::Create => fs_event(action, FsOp::Create, path, &cap),
        OpClass::Delete => fs_event(action, FsOp::Delete, path, &cap),
        OpClass::Exec => match path {
            Some(p) => SydParseOutcome::Event(AccessEvent {
                action,
                event: InferEvent::Exec {
                    path: PathBuf::from(p),
                },
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

/// Normalize syd's `cap` field — a string or an array of strings — into
/// a single best-match category. When multiple recognized categories
/// appear together (e.g. `["write","create"]` for an O_CREAT|O_WRONLY
/// open), the more specific one wins (Create > Delete > Write > Read >
/// Stat > ReadDir > Exec). Falls back to the array's first element so
/// unrecognized caps still surface in the Unsupported diagnostic.
fn normalize_cap(value: Option<&serde_json::Value>) -> String {
    match value {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => {
            let strs: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
            const PRIORITY: &[&str] = &[
                "create", "truncate", "delete", "write", "read", "stat", "readdir", "exec", "fs",
            ];
            for &want in PRIORITY {
                if strs.contains(&want) {
                    return want.to_string();
                }
            }
            strs.first().map(|s| s.to_string()).unwrap_or_default()
        }
        _ => String::new(),
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

fn fs_event(
    action: AccessAction,
    op: FsOp,
    path: Option<&str>,
    operation: &str,
) -> SydParseOutcome {
    match path {
        Some(p) => SydParseOutcome::Event(AccessEvent {
            action,
            event: InferEvent::Fs {
                op,
                path: PathBuf::from(p),
            },
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

    fn fs(action: AccessAction, op: FsOp, path: &str) -> SydParseOutcome {
        SydParseOutcome::Event(AccessEvent {
            action,
            event: InferEvent::Fs { op, path: p(path) },
        })
    }

    fn exec(action: AccessAction, path: &str) -> SydParseOutcome {
        SydParseOutcome::Event(AccessEvent {
            action,
            event: InferEvent::Exec { path: p(path) },
        })
    }

    #[test]
    fn read_op_maps_to_fs_read() {
        assert_eq!(
            parse_access_line(&line("read", "/etc/hosts")),
            fs(AccessAction::Warn, FsOp::Read, "/etc/hosts")
        );
    }

    #[test]
    fn open_and_openat_map_to_fs_read() {
        for op in ["open", "openat"] {
            assert_eq!(
                parse_access_line(&line(op, "/etc/hosts")),
                fs(AccessAction::Warn, FsOp::Read, "/etc/hosts"),
                "operation {op}"
            );
        }
    }

    #[test]
    fn readlink_maps_to_read() {
        assert!(matches!(
            parse_access_line(&line("readlink", "/proc/self/exe")),
            SydParseOutcome::Event(AccessEvent {
                event: InferEvent::Fs { op: FsOp::Read, .. },
                ..
            })
        ));
    }

    #[test]
    fn stat_family_maps_to_fs_stat() {
        for op in ["stat", "lstat", "fstatat", "access", "faccessat"] {
            assert!(
                matches!(
                    parse_access_line(&line(op, "/etc/hosts")),
                    SydParseOutcome::Event(AccessEvent {
                        event: InferEvent::Fs { op: FsOp::Stat, .. },
                        ..
                    })
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
                    parse_access_line(&line(op, "/etc")),
                    SydParseOutcome::Event(AccessEvent {
                        event: InferEvent::Fs {
                            op: FsOp::ReadDir,
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
    fn write_maps_to_fs_write() {
        assert_eq!(
            parse_access_line(&line("write", "/tmp/work/out.txt")),
            fs(AccessAction::Warn, FsOp::Write, "/tmp/work/out.txt")
        );
    }

    #[test]
    fn create_family_maps_to_fs_create() {
        for op in ["create", "truncate"] {
            assert!(
                matches!(
                    parse_access_line(&line(op, "/tmp/work/out.txt")),
                    SydParseOutcome::Event(AccessEvent {
                        event: InferEvent::Fs {
                            op: FsOp::Create,
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
    fn delete_cap_maps_to_fs_delete() {
        assert!(matches!(
            parse_access_line(&line("delete", "/tmp/work/old.txt")),
            SydParseOutcome::Event(AccessEvent {
                event: InferEvent::Fs {
                    op: FsOp::Delete,
                    ..
                },
                ..
            })
        ));
    }

    #[test]
    fn exec_cap_maps_to_exec_event() {
        assert_eq!(
            parse_access_line(&line("exec", "/usr/bin/ls")),
            exec(AccessAction::Warn, "/usr/bin/ls")
        );
    }

    #[test]
    fn unknown_operation_is_unsupported_with_op_name() {
        let outcome = parse_access_line(&line("ioctl", "/dev/null"));
        let SydParseOutcome::Unsupported(d) = outcome else {
            panic!("expected Unsupported, got {outcome:?}");
        };
        assert!(d.message.contains("ioctl"), "message: {}", d.message);
        assert_eq!(d.level, DiagnosticLevel::Warn);
    }

    #[test]
    fn non_access_ctx_is_skipped() {
        let line = r#"{"ctx":"run","op":"boot","msg":"x"}"#;
        assert_eq!(parse_access_line(line), SydParseOutcome::Skip);
    }

    #[test]
    fn syd_act_allow_classifies_as_allow() {
        let line = r#"{"ctx":"access","cap":"read","act":"allow","path":"/etc/hosts"}"#;
        assert_eq!(
            parse_access_line(line),
            fs(AccessAction::Allow, FsOp::Read, "/etc/hosts")
        );
    }

    #[test]
    fn syd_act_warn_classifies_as_warn() {
        let line = r#"{"ctx":"access","cap":"read","act":"warn","path":"/etc/hosts"}"#;
        assert_eq!(
            parse_access_line(line),
            fs(AccessAction::Warn, FsOp::Read, "/etc/hosts")
        );
    }

    #[test]
    fn syd_act_deny_classifies_as_deny() {
        let line = r#"{"ctx":"access","cap":"read","act":"deny","path":"/etc/secret"}"#;
        assert_eq!(
            parse_access_line(line),
            fs(AccessAction::Deny, FsOp::Read, "/etc/secret")
        );
    }

    #[test]
    fn syd_unrecognized_act_is_skipped() {
        let line = r#"{"ctx":"access","cap":"read","act":"weird","path":"/etc/hosts"}"#;
        assert_eq!(parse_access_line(line), SydParseOutcome::Skip);
    }

    #[test]
    fn syd_cap_array_normalizes_to_create() {
        // syd 3.49.1 sometimes emits cap as an array for compound
        // operations (e.g. an O_CREAT|O_WRONLY open). Create wins over
        // Write because it's the more specific classification.
        let line =
            r#"{"ctx":"access","cap":["write","create"],"act":"deny","path":"/tmp/work/new.txt"}"#;
        assert_eq!(
            parse_access_line(line),
            fs(AccessAction::Deny, FsOp::Create, "/tmp/work/new.txt")
        );
    }

    #[test]
    fn syd_cap_array_with_only_write_normalizes_to_write() {
        let line = r#"{"ctx":"access","cap":["write"],"act":"warn","path":"/tmp/work/out.txt"}"#;
        assert_eq!(
            parse_access_line(line),
            fs(AccessAction::Warn, FsOp::Write, "/tmp/work/out.txt")
        );
    }

    #[test]
    fn missing_ctx_field_is_malformed() {
        let line = r#"{"cap":"read","path":"/etc/hosts"}"#;
        assert!(matches!(
            parse_access_line(line),
            SydParseOutcome::Malformed(_)
        ));
    }

    #[test]
    fn missing_path_for_fs_op_is_malformed() {
        let line = r#"{"ctx":"access","cap":"read","act":"warn"}"#;
        assert!(matches!(
            parse_access_line(line),
            SydParseOutcome::Malformed(_)
        ));
    }

    #[test]
    fn missing_path_for_exec_is_malformed() {
        let line = r#"{"ctx":"access","cap":"exec","act":"warn"}"#;
        assert!(matches!(
            parse_access_line(line),
            SydParseOutcome::Malformed(_)
        ));
    }

    #[test]
    fn malformed_json_is_malformed() {
        let outcome = parse_access_line("{not json");
        let SydParseOutcome::Malformed(d) = outcome else {
            panic!("expected Malformed, got {outcome:?}");
        };
        assert!(d.message.contains("invalid JSON"), "{}", d.message);
    }

    #[test]
    fn empty_line_is_skipped() {
        assert_eq!(parse_access_line(""), SydParseOutcome::Skip);
    }

    #[test]
    fn whitespace_only_line_is_skipped() {
        assert_eq!(parse_access_line("   \t  \n"), SydParseOutcome::Skip);
    }

    #[test]
    fn unknown_extra_fields_are_tolerated() {
        let line = r#"{"ctx":"access","cap":"read","act":"warn","path":"/etc/hosts","pid":1234,"future_field":{"nested":true},"another":42}"#;
        assert_eq!(
            parse_access_line(line),
            fs(AccessAction::Warn, FsOp::Read, "/etc/hosts")
        );
    }

    #[test]
    fn real_syd_3_49_records_parse() {
        // Shape verified empirically against syd 3.49.1 on Linux:
        // ctx="access", cap=<category>, act="warn"|"deny", path=<abs>.
        let samples = [
            (
                r#"{"id":"x","syd":1,"ctx":"access","cap":"read","act":"warn","sys":"openat","fs":"ext","path":"/etc/hosts","mode":0,"oflags":["cloexec"],"type":"reg","time":"t","pid":2,"uid":1000}"#,
                fs(AccessAction::Warn, FsOp::Read, "/etc/hosts"),
            ),
            (
                r#"{"id":"x","ctx":"access","cap":"stat","act":"warn","sys":"stat","path":"/usr/lib/libc.so.6","args":[0,0,0,0,0,0],"time":"t","pid":2,"uid":1000}"#,
                fs(AccessAction::Warn, FsOp::Stat, "/usr/lib/libc.so.6"),
            ),
            (
                r#"{"id":"x","ctx":"access","cap":"exec","act":"warn","sys":"execve","path":"/usr/bin/ls","time":"t","pid":2,"uid":1000}"#,
                exec(AccessAction::Warn, "/usr/bin/ls"),
            ),
            (
                r#"{"id":"x","ctx":"access","cap":"create","act":"warn","sys":"openat","path":"/tmp/work/out.txt","time":"t","pid":2,"uid":1000}"#,
                fs(AccessAction::Warn, FsOp::Create, "/tmp/work/out.txt"),
            ),
            (
                r#"{"id":"x","ctx":"access","cap":"delete","act":"warn","sys":"unlinkat","path":"/tmp/work/old.txt","time":"t","pid":2,"uid":1000}"#,
                fs(AccessAction::Warn, FsOp::Delete, "/tmp/work/old.txt"),
            ),
            (
                r#"{"id":"x","ctx":"access","cap":"readdir","act":"warn","sys":"getdents64","path":"/etc","time":"t","pid":2,"uid":1000}"#,
                fs(AccessAction::Warn, FsOp::ReadDir, "/etc"),
            ),
        ];

        for (raw, expected) in samples {
            assert_eq!(parse_access_line(raw), expected, "line: {raw}");
        }
    }

    #[test]
    fn trailing_newline_is_stripped() {
        let line = format!("{}\n", line("read", "/etc/hosts"));
        assert!(matches!(
            parse_access_line(&line),
            SydParseOutcome::Event(AccessEvent {
                event: InferEvent::Fs { op: FsOp::Read, .. },
                ..
            })
        ));
    }
}
