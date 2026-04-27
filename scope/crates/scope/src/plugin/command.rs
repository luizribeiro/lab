use std::process::Stdio;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

pub struct PluginRunner {
    command: Vec<String>,
    timeout: Duration,
}

impl PluginRunner {
    pub fn new(command: Vec<String>, timeout: Duration) -> Self {
        Self { command, timeout }
    }

    pub async fn run<Req: Serialize, Res: DeserializeOwned>(&self, req: &Req) -> Result<Res> {
        let (program, args) = self
            .command
            .split_first()
            .ok_or_else(|| anyhow!("plugin command is empty"))?;

        let mut child = Command::new(program)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("failed to spawn plugin {program}"))?;

        let payload = serde_json::to_vec(req).context("failed to serialize plugin request")?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("failed to open plugin stdin"))?;
        stdin
            .write_all(&payload)
            .await
            .context("failed to write plugin request")?;
        drop(stdin);

        let output = match timeout(self.timeout, child.wait_with_output()).await {
            Ok(result) => result.context("failed to wait for plugin")?,
            Err(_) => {
                return Err(anyhow!(
                    "plugin {program} timed out after {:?}",
                    self.timeout
                ));
            }
        };

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "plugin {program} exited with {}: {}",
                output.status,
                stderr.trim()
            ));
        }

        let stdout = std::str::from_utf8(&output.stdout)
            .context("plugin stdout was not valid UTF-8")?;
        serde_json::from_str(stdout)
            .with_context(|| format!("failed to parse plugin {program} response as JSON"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[derive(Serialize)]
    struct Req {
        msg: String,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    struct Res {
        echo: String,
    }

    fn write_script(dir: &TempDir, body: &str) -> PathBuf {
        let path = dir.path().join("plugin.sh");
        std::fs::write(&path, body).unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        path
    }

    fn runner(path: PathBuf, secs: u64) -> PluginRunner {
        PluginRunner::new(
            vec![path.to_string_lossy().into_owned()],
            Duration::from_secs(secs),
        )
    }

    #[tokio::test]
    async fn round_trip_success() {
        let dir = TempDir::new().unwrap();
        let path = write_script(
            &dir,
            "#!/bin/sh\ncat > /dev/null\nprintf '{\"echo\":\"hi\"}'\n",
        );
        let runner = runner(path, 5);
        let res: Res = runner.run(&Req { msg: "hello".into() }).await.unwrap();
        assert_eq!(res, Res { echo: "hi".into() });
    }

    #[tokio::test]
    async fn nonzero_exit_includes_stderr() {
        let dir = TempDir::new().unwrap();
        let path = write_script(&dir, "#!/bin/sh\ncat > /dev/null\necho boom 1>&2\nexit 1\n");
        let runner = runner(path, 5);
        let err = runner
            .run::<Req, Res>(&Req { msg: "x".into() })
            .await
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("boom"), "unexpected: {msg}");
    }

    #[tokio::test]
    async fn malformed_json_errors() {
        let dir = TempDir::new().unwrap();
        let path = write_script(
            &dir,
            "#!/bin/sh\ncat > /dev/null\nprintf 'not json'\n",
        );
        let runner = runner(path, 5);
        let err = runner
            .run::<Req, Res>(&Req { msg: "x".into() })
            .await
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("JSON") || msg.contains("parse"), "unexpected: {msg}");
    }

    #[tokio::test]
    async fn timeout_kills_child() {
        let dir = TempDir::new().unwrap();
        let path = write_script(&dir, "#!/bin/sh\nsleep 5\n");
        let runner = PluginRunner::new(
            vec![path.to_string_lossy().into_owned()],
            Duration::from_millis(150),
        );
        let err = runner
            .run::<Req, Res>(&Req { msg: "x".into() })
            .await
            .unwrap_err();
        let msg = format!("{err:#}");
        assert!(msg.contains("timed out"), "unexpected: {msg}");
    }

    #[tokio::test]
    async fn handles_large_stdout() {
        let dir = TempDir::new().unwrap();
        let big = "x".repeat(64 * 1024);
        let script = format!(
            "#!/bin/sh\ncat > /dev/null\nprintf '{{\"echo\":\"{big}\"}}'\n"
        );
        let path = write_script(&dir, &script);
        let runner = runner(path, 10);
        let res: Res = runner.run(&Req { msg: "x".into() }).await.unwrap();
        assert_eq!(res.echo.len(), 64 * 1024);
    }
}
