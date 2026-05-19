use std::path::{Path, PathBuf};

/// Falls back to `/tmp/.orb` when `$HOME` is unset so the app can keep
/// going rather than crash.
pub fn orb_data_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(".orb")
}

pub fn prompt_history_path() -> PathBuf {
    // When `$HOME` is unset, fall back to a cwd-local file rather than a
    // shared `/tmp` location: prompt history is private and we'd
    // rather not blend it with another user's.
    dirs::home_dir()
        .map(|_| orb_data_dir().join("history"))
        .unwrap_or_else(|| PathBuf::from(".orb-history"))
}

pub fn transcripts_dir() -> PathBuf {
    orb_data_dir().join("transcripts")
}

pub fn abbreviate_home(path: &Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rel) = path.strip_prefix(&home)
    {
        return format!("~/{}", rel.display());
    }
    path.display().to_string()
}

pub fn git_branch(cwd: &Path) -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(cwd)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_string();
    if s.is_empty() || s == "HEAD" {
        None
    } else {
        Some(s)
    }
}
