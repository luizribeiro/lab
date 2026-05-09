//! Env scrubber per scope §Sc1–§Sc3 (security RFC §7.4).
//!
//! `strip` removes secret-pattern matches from an `env.pass` list.
//! `reject_reserved` enforces §C7.1: the compiler rejects any
//! request to forward or override `RFL_BUS_FD` / `RFL_PLUGIN`.

use std::collections::BTreeMap;

use crate::error::CompileError;

/// v1 secret-pattern glob set per scope §Sc1.
pub const SECRET_PATTERNS: &[&str] = &[
    "*_TOKEN",
    "*_SECRET",
    "*_KEY",
    "*_PASSWORD",
    "AWS_*",
    "GITHUB_TOKEN",
    "OPENAI_*",
    "ANTHROPIC_*",
];

const RESERVED_ENV_VARS: &[&str] = &["RFL_BUS_FD", "RFL_PLUGIN"];

/// Scope §Sc2: scrub an `env.pass` list against `SECRET_PATTERNS`.
/// With `i_know_what_im_doing == true`, returns `env_pass` verbatim.
pub fn strip(env_pass: &[String], i_know_what_im_doing: bool) -> Vec<String> {
    if i_know_what_im_doing {
        return env_pass.to_vec();
    }
    env_pass
        .iter()
        .filter(|name| !is_secret(name))
        .cloned()
        .collect()
}

/// Scope §C7.1: reject any request to pass or set the core-owned
/// `RFL_BUS_FD` / `RFL_PLUGIN` env vars.
pub fn reject_reserved(
    env_pass: &[String],
    env_set: &BTreeMap<String, String>,
) -> Result<(), CompileError> {
    for name in env_pass {
        if is_reserved(name) {
            return Err(CompileError::ReservedEnvVarRequested);
        }
    }
    for key in env_set.keys() {
        if is_reserved(key) {
            return Err(CompileError::ReservedEnvVarRequested);
        }
    }
    Ok(())
}

fn is_reserved(name: &str) -> bool {
    RESERVED_ENV_VARS.contains(&name)
}

fn is_secret(name: &str) -> bool {
    SECRET_PATTERNS.iter().any(|pat| glob_match(pat, name))
}

fn glob_match(pattern: &str, name: &str) -> bool {
    match (pattern.starts_with('*'), pattern.ends_with('*')) {
        (true, true) => {
            let inner = &pattern[1..pattern.len() - 1];
            name.contains(inner)
        }
        (true, false) => {
            let suffix = &pattern[1..];
            name.ends_with(suffix)
        }
        (false, true) => {
            let prefix = &pattern[..pattern.len() - 1];
            name.starts_with(prefix)
        }
        (false, false) => pattern == name,
    }
}
