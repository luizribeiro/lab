use std::io::{self, Write};

pub(super) fn run_editor(initial: &str) -> io::Result<String> {
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());
    let mut tmp = tempfile::Builder::new()
        .prefix("orb-prompt-")
        .suffix(".md")
        .tempfile()?;
    tmp.write_all(initial.as_bytes())?;
    tmp.flush()?;
    let (file, path) = tmp.keep().map_err(io::Error::other)?;
    drop(file);
    let status = std::process::Command::new(&editor).arg(&path).status()?;
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let _ = std::fs::remove_file(&path);
    if !status.success() {
        return Err(io::Error::other("editor exited non-zero"));
    }
    Ok(content.trim_end_matches('\n').to_string())
}
