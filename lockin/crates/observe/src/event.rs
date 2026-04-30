//! Cross-platform event model produced by policy-inference backends.

use std::path::PathBuf;

/// A single observed access made by the audited program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InferEvent {
    /// Filesystem access on a path.
    Fs { op: FsOp, path: PathBuf },
    /// Process exec of a binary.
    Exec { path: PathBuf },
    /// Backend observed something the inference layer can't translate
    /// into the current lockin schema (mach lookup, sysctl, ioctl, etc.).
    /// These surface as warnings; they never become policy.
    Unsupported {
        backend: &'static str,
        raw: String,
        reason: String,
    },
}

/// Filesystem operation classes that map onto the `[filesystem]` schema.
/// Backends classify their native syscalls/operations into one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FsOp {
    /// Open/read file contents.
    Read,
    /// stat/lstat/access/readlink — metadata only, no contents.
    Stat,
    /// readdir on a directory.
    ReadDir,
    /// Write to an existing file.
    Write,
    /// Create a new file or truncate to length 0.
    Create,
    /// Unlink/rmdir on an existing path.
    Delete,
}

/// What the sandbox decided about an observed access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessAction {
    /// Sandbox permitted the access. Emitted in
    /// `ObservationMode::AllowAllWithRunId` mode (current `lockin infer`).
    Allow,
    /// Sandbox would have denied but allowed (dry-run / warn). Emitted
    /// when default rules are set to `:warn`.
    Warn,
    /// Sandbox denied the access. Emitted when default rules are set to
    /// `:deny` (used by `lockin trace`).
    Deny,
}

/// An observed access decorated with the sandbox's decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessEvent {
    pub action: AccessAction,
    pub event: InferEvent,
}

/// A diagnostic surfaced by inference (informational, warning, or error).
/// Used to propagate things like "saw an unsupported sandbox operation"
/// up to the CLI without crashing the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferDiagnostic {
    pub level: DiagnosticLevel,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Info,
    Warn,
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn fs_event_round_trips_clone_and_eq() {
        let original = InferEvent::Fs {
            op: FsOp::Read,
            path: PathBuf::from("/etc/hosts"),
        };
        let clone = original.clone();
        assert_eq!(original, clone);
    }

    #[test]
    fn fs_op_is_copy_and_hashable() {
        let a: FsOp = FsOp::Read;
        let _b = a; // Copy: original still usable below.
        let mut set: HashSet<FsOp> = HashSet::new();
        set.insert(a);
        set.insert(FsOp::Write);
        set.insert(FsOp::Read); // duplicate, no-op
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn diagnostic_constructs_and_compares() {
        let d = InferDiagnostic {
            level: DiagnosticLevel::Warn,
            message: "unsupported op".into(),
        };
        assert_eq!(d.level, DiagnosticLevel::Warn);
        assert_ne!(d.level, DiagnosticLevel::Error);
        assert_eq!(d, d.clone());
    }
}
