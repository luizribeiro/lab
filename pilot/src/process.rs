//! Spawn a Driver command and stream JSONL values from its stdout.

use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;

use crate::Error;
use crate::driver::CommandSpec;

pub(crate) struct ProcessHandle {
    task: tokio::task::JoinHandle<()>,
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(crate) async fn spawn_jsonl(
    spec: CommandSpec,
    workdir: std::path::PathBuf,
) -> crate::Result<(
    ProcessHandle,
    mpsc::Receiver<crate::Result<serde_json::Value>>,
)> {
    let mut cmd = Command::new(&spec.program);
    cmd.args(&spec.args).current_dir(&workdir);
    for (k, v) in &spec.env {
        cmd.env(k, v);
    }
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.kill_on_drop(true);

    let mut child = cmd.spawn().map_err(Error::Spawn)?;
    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");

    // bounded channel; slow consumers back-pressure the child via stdout pipe.
    let (tx, rx) = mpsc::channel(256);

    let handle = tokio::spawn(async move {
        let stderr_task = tokio::spawn(async move {
            let mut buf = String::new();
            let _ = BufReader::new(stderr).read_to_string(&mut buf).await;
            buf
        });

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
                        break;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    let _ = tx.send(Err(Error::Io(e))).await;
                    break;
                }
            }
        }

        let stderr_buf = stderr_task.await.unwrap_or_default();
        #[allow(clippy::collapsible_if)]
        if let Ok(status) = child.wait().await {
            if !status.success() {
                let _ = tx
                    .send(Err(Error::Exit {
                        status,
                        stderr: stderr_buf,
                    }))
                    .await;
            }
        }
    });

    Ok((ProcessHandle { task: handle }, rx))
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
        p.push(format!("fake_agent{}", std::env::consts::EXE_SUFFIX));
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

        let (_h, mut rx) = spawn_jsonl(spec_running(script.path()), std::env::temp_dir())
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

        let (_h, mut rx) = spawn_jsonl(spec_running(script.path()), std::env::temp_dir())
            .await
            .expect("spawn");

        let err = rx.recv().await.unwrap().unwrap_err();
        assert!(matches!(err, crate::Error::Parse { .. }));
    }

    #[tokio::test]
    async fn nonzero_exit_surfaces_exit_error_with_stderr() {
        let mut script = NamedTempFile::new().unwrap();
        writeln!(script, "stderr something went wrong").unwrap();
        writeln!(script, "exit 7").unwrap();
        script.flush().unwrap();

        let (_h, mut rx) = spawn_jsonl(spec_running(script.path()), std::env::temp_dir())
            .await
            .expect("spawn");

        let mut last_err = None;
        while let Some(item) = rx.recv().await {
            if let Err(e) = item {
                last_err = Some(e);
            }
        }
        let err = last_err.expect("exit error surfaced");
        match err {
            crate::Error::Exit { status, stderr } => {
                assert_eq!(status.code(), Some(7));
                assert!(
                    stderr.contains("something went wrong"),
                    "stderr captured: {stderr:?}"
                );
            }
            other => panic!("expected Exit, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn clean_exit_yields_no_extra_error() {
        let mut script = NamedTempFile::new().unwrap();
        writeln!(script, r#"emit {{"n":1}}"#).unwrap();
        writeln!(script, "exit 0").unwrap();
        script.flush().unwrap();

        let (_h, mut rx) = spawn_jsonl(spec_running(script.path()), std::env::temp_dir())
            .await
            .expect("spawn");

        let mut count = 0;
        let mut saw_err = false;
        while let Some(item) = rx.recv().await {
            match item {
                Ok(_) => count += 1,
                Err(_) => saw_err = true,
            }
        }
        assert_eq!(count, 1);
        assert!(!saw_err, "clean exit must not surface any Error");
    }
}
