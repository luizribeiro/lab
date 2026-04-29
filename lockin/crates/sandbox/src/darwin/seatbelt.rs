use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
enum SeatbeltFilter {
    Literal(PathBuf),
    Subpath(PathBuf),
}

#[derive(Debug, Clone)]
struct SeatbeltRule {
    operations: Vec<&'static str>,
    filter: Option<SeatbeltFilter>,
}

impl SeatbeltRule {
    fn allow(operations: &[&'static str]) -> Self {
        Self {
            operations: operations.to_vec(),
            filter: None,
        }
    }

    fn allow_literal(operations: &[&'static str], path: &Path) -> Self {
        Self {
            operations: operations.to_vec(),
            filter: Some(SeatbeltFilter::Literal(path.to_path_buf())),
        }
    }

    fn allow_subpath(operations: &[&'static str], path: &Path) -> Self {
        Self {
            operations: operations.to_vec(),
            filter: Some(SeatbeltFilter::Subpath(path.to_path_buf())),
        }
    }

    fn render(&self) -> String {
        let mut out = String::from("(allow");

        for op in &self.operations {
            out.push(' ');
            out.push_str(op);
        }

        if let Some(filter) = &self.filter {
            out.push(' ');
            match filter {
                SeatbeltFilter::Literal(path) => {
                    out.push_str("(literal \"");
                    out.push_str(&escape_sb_string(path));
                    out.push_str("\")");
                }
                SeatbeltFilter::Subpath(path) => {
                    out.push_str("(subpath \"");
                    out.push_str(&escape_sb_string(path));
                    out.push_str("\")");
                }
            }
        }

        out.push_str(")\n");
        out
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct SeatbeltPolicy {
    imports: Vec<String>,
    rules: Vec<SeatbeltRule>,
    raw_rules: Vec<String>,
    default_clause: Option<String>,
}

impl SeatbeltPolicy {
    /// Overrides the leading `(deny default)` clause. The provided
    /// string is emitted verbatim as the catch-all rule.
    pub(super) fn set_default_clause(&mut self, clause: impl Into<String>) {
        self.default_clause = Some(clause.into());
    }

    pub(super) fn import_system(&mut self) {
        let profile = "system.sb".to_string();
        if !self.imports.iter().any(|existing| existing == &profile) {
            self.imports.push(profile);
        }
    }

    pub(super) fn allow(&mut self, operations: &[&'static str]) {
        self.rules.push(SeatbeltRule::allow(operations));
    }

    pub(super) fn allow_literal(&mut self, operations: &[&'static str], path: &Path) {
        self.rules
            .push(SeatbeltRule::allow_literal(operations, path));
    }

    pub(super) fn allow_subpath(&mut self, operations: &[&'static str], path: &Path) {
        self.rules
            .push(SeatbeltRule::allow_subpath(operations, path));
    }

    pub(super) fn append_raw(&mut self, rule: impl Into<String>) {
        self.raw_rules.push(rule.into());
    }
}

impl From<SeatbeltPolicy> for String {
    fn from(policy: SeatbeltPolicy) -> Self {
        let mut out = String::new();
        out.push_str("(version 1)\n");
        match policy.default_clause {
            Some(clause) => {
                out.push_str(&clause);
                if !clause.ends_with('\n') {
                    out.push('\n');
                }
            }
            None => out.push_str("(deny default)\n"),
        }

        for import in policy.imports {
            out.push_str("(import \"");
            out.push_str(&escape_sb_text(&import));
            out.push_str("\")\n");
        }

        for rule in policy.rules {
            out.push_str(&rule.render());
        }

        for raw in policy.raw_rules {
            out.push_str(&raw);
            if !raw.ends_with('\n') {
                out.push('\n');
            }
        }

        out
    }
}

fn escape_sb_string(path: &Path) -> String {
    escape_sb_text(&path.display().to_string())
}

fn escape_sb_text(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
