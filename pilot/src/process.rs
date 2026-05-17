//! Spawn a Driver command and stream JSONL values from its stdout.

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;

use crate::Error;
use crate::driver::CommandSpec;

#[allow(dead_code)] // wired into Session in a later commit
pub(crate) async fn spawn_jsonl(
    spec: CommandSpec,
    workdir: std::path::PathBuf,
) -> crate::Result<(Child, mpsc::Receiver<crate::Result<serde_json::Value>>)> {
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args).current_dir(&workdir);
    for (k, v) in &spec.env {
        cmd.env(k, v);
    }
    cmd.stdout(std::process::Stdio::piped());

    let mut child = cmd.spawn().map_err(Error::Spawn)?;
    let stdout = child.stdout.take().expect("stdout piped");

    // bounded so a slow consumer back-pressures the child.
    let (tx, rx) = mpsc::channel(256);

    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        loop {
            match lines.next_line().await {
                Ok(Some(line)) => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    let msg = match serde_json::from_str::<serde_json::Value>(&line) {
                        Ok(v) => Ok(v),
                        Err(source) => Err(Error::Parse { line, source }),
                    };
                    if tx.send(msg).await.is_err() {
                        return;
                    }
                }
                Ok(None) => return,
                Err(e) => {
                    let _ = tx.send(Err(Error::Io(e))).await;
                    return;
                }
            }
        }
    });

    Ok((child, rx))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    fn fake_agent() -> PathBuf {
        let mut p = std::env::current_exe().unwrap();
        p.pop();
        if p.ends_with("deps") {
            p.pop();
        }
        p.push("fake_agent");
        p
    }

    fn spec_running(script_path: &std::path::Path) -> CommandSpec {
        CommandSpec {
            program: fake_agent(),
            args: vec!["--script".into(), script_path.to_string_lossy().into()],
            env: vec![],
        }
    }

    #[tokio::test]
    async fn spawns_and_streams_lines_in_order() {
        let mut script = NamedTempFile::new().unwrap();
        writeln!(script, r#"emit {{"n":1}}"#).unwrap();
        writeln!(script, r#"emit {{"n":2}}"#).unwrap();
        script.flush().unwrap();

        let (_child, mut rx) = spawn_jsonl(spec_running(script.path()), std::env::temp_dir())
            .await
            .expect("spawn");

        let a = rx.recv().await.unwrap().unwrap();
        let b = rx.recv().await.unwrap().unwrap();
        assert_eq!(a["n"], 1);
        assert_eq!(b["n"], 2);
        assert!(rx.recv().await.is_none(), "channel closes at EOF");
    }

    #[tokio::test]
    async fn malformed_line_surfaces_parse_error() {
        let mut script = NamedTempFile::new().unwrap();
        writeln!(script, "emit not-json").unwrap();
        script.flush().unwrap();

        let (_child, mut rx) = spawn_jsonl(spec_running(script.path()), std::env::temp_dir())
            .await
            .expect("spawn");

        let err = rx.recv().await.unwrap().unwrap_err();
        assert!(matches!(err, crate::Error::Parse { .. }));
    }
}
